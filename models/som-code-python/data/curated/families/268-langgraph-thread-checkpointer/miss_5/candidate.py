"""A LangGraph-style in-memory checkpointer keyed by thread_id."""

from __future__ import annotations

import copy
from collections.abc import Callable, Mapping
from typing import Any

State = dict[str, Any]
Config = Mapping[str, Any]


def thread_id_of(config: Config) -> str:
    """Return config["configurable"]["thread_id"] or raise ValueError."""
    configurable = config.get("configurable", {})
    thread_id = configurable.get("thread_id")
    if not isinstance(thread_id, str) or not thread_id:
        raise ValueError("config needs configurable.thread_id")
    return thread_id


class MemorySaver:
    """Store deep-copied snapshots per thread, newest last."""

    def __init__(self) -> None:
        self._threads: dict[str, list[State]] = {}

    def put(self, config: Config, state: State) -> None:
        """Append a snapshot of state to the thread's history."""
        self._threads.setdefault(thread_id_of(config), []).append(copy.deepcopy(state))

    def get(self, config: Config) -> State | None:
        """Return a copy of the latest snapshot, or None for a new thread."""
        history = self._threads.get(thread_id_of(config))
        return copy.deepcopy(history[-1]) if history else None

    def history(self, config: Config) -> list[State]:
        """Return copies of every snapshot, oldest first."""
        return copy.deepcopy(self._threads.get(thread_id_of(config), []))


class ChatGraph:
    """Append each user turn and a reply to the thread's saved messages."""

    def __init__(self, reply: Callable[[list[str]], str], saver: MemorySaver) -> None:
        self._reply = reply
        self._saver = saver

    def invoke(self, message: str, config: Config) -> State:
        """Resume the thread, add the turn, checkpoint and return the state."""
        state = self._saver.get(config) or {"messages": [], "turns": 0}
        messages = [*state["messages"], f"user: {message}"]
        messages.append(f"ai: {self._reply(messages)}")
        state = {"messages": messages, "turns": state["turns"] + 1}
        self._saver.put(config, state)
        return state

    def snapshot_count(self, config: Config) -> int:
        """Return how many checkpoints the thread has."""
        return len(self._saver.history(config))
