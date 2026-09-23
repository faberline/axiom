"""Bound per-session chat memory, keep the system prompt, and evict LRU sessions."""

from __future__ import annotations

from collections import OrderedDict
from dataclasses import dataclass


@dataclass(frozen=True)
class Message:
    """One chat message with its role."""

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
        """Return the session's history, creating it and evicting the LRU session."""
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
        """Append a human message and prune the session."""
        history = self._ensure_session(session_id)
        history.append(Message(role="human", content=content))
        self.prune_history(session_id)

    def add_ai_message(self, session_id: str, content: str) -> None:
        """Append an AI message and prune the session."""
        history = self._ensure_session(session_id)
        history.append(Message(role="ai", content=content))
        self.prune_history(session_id)

    def get_messages(self, session_id: str) -> list[dict[str, str]]:
        """Return the session's messages as dicts and mark it recently used."""
        if session_id not in self._sessions:
            return []
        self._sessions.move_to_end(session_id)
        return [
            {"role": m.role, "content": m.content} for m in self._sessions[session_id]
        ]

    def prune_history(self, session_id: str) -> int:
        """Trim dialogue to max_messages, keeping the system message; return trimmed."""
        if session_id not in self._sessions:
            return 0
        history = self._sessions[session_id]
        has_system = len(history) > 0 and history[0].role == "system"

        if has_system:
            system_msg = history[0]
            dialogue = history[1:]
            if len(dialogue) > self.max_messages:
                pruned_count = len(dialogue) - self.max_messages
                self._sessions[session_id] = dialogue[-self.max_messages :]
                return pruned_count
        elif len(history) > self.max_messages:
            pruned_count = len(history) - self.max_messages
            self._sessions[session_id] = history[-self.max_messages :]
            return pruned_count

        return 0

    def clear_expired_sessions(self, max_allowed: int | None = None) -> int:
        """Evict least recently used sessions beyond the cap; return how many."""
        cap = max_allowed if max_allowed is not None else self.max_sessions
        evicted = 0
        while len(self._sessions) > cap:
            self._sessions.popitem(last=False)
            evicted += 1
        return evicted

    def get_session_count(self) -> int:
        """Return the number of live sessions."""
        return len(self._sessions)
