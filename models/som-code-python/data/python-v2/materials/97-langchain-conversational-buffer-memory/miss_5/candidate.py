"""SlidingWindowMemoryBuffer with System Message Preservation and LRU Eviction."""
from __future__ import annotations

from collections import OrderedDict
from dataclasses import dataclass
from typing import Any


@dataclass(frozen=True)
class Message:
    role: str
    content: str


class SlidingWindowMemoryBuffer:
    """Manages conversational memory with strict windowing and session bounds."""

    def __init__(
        self,
        max_messages: int = 10,
        max_sessions: int = 100,
        system_prompt: str | None = None,
    ) -> None:
        self.max_messages = max_messages
        self.max_sessions = max_sessions
        self.system_prompt = system_prompt
        self._sessions: OrderedDict[str, list[Message]] = OrderedDict()

    def _ensure_session(self, session_id: str) -> list[Message]:
        if session_id in self._sessions:
            self._sessions.move_to_end(session_id)
            return self._sessions[session_id]

        if len(self._sessions) >= self.max_sessions:
            self._sessions.popitem(last=False)

        history: list[Message] = []
        if self.system_prompt is not None:
            history.append(Message(role="system", content=self.system_prompt))
        self._sessions[session_id] = history
        return history

    def add_user_message(self, session_id: str, content: str) -> None:
        history = self._ensure_session(session_id)
        history.append(Message(role="human", content=content))
        self.prune_history(session_id)

    def add_ai_message(self, session_id: str, content: str) -> None:
        history = self._ensure_session(session_id)
        history.append(Message(role="ai", content=content))
        self.prune_history(session_id)

    def get_messages(self, session_id: str) -> list[dict[str, str]]:
        if session_id not in self._sessions:
            return []
        self._sessions.move_to_end(session_id)
        return [{"role": m.role, "content": m.content} for m in self._sessions[session_id]]

    def prune_history(self, session_id: str) -> int:
        if session_id not in self._sessions:
            return 0
        history = self._sessions[session_id]
        has_system = len(history) > 0 and history[0].role == "system"

        if has_system:
            system_msg = history[0]
            dialogue = history[1:]
            if len(dialogue) > self.max_messages:
                pruned_count = len(dialogue) - self.max_messages
                return pruned_count
        else:
            if len(history) > self.max_messages:
                pruned_count = len(history) - self.max_messages
                return pruned_count

        return 0

    def clear_expired_sessions(self, max_allowed: int | None = None) -> int:
        cap = max_allowed if max_allowed is not None else self.max_sessions
        evicted = 0
        while len(self._sessions) > cap:
            self._sessions.popitem(last=False)
            evicted += 1
        return evicted

    def get_session_count(self) -> int:
        return len(self._sessions)
