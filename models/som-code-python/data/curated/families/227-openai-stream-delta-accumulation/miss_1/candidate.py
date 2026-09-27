"""Accumulate streamed OpenAI chat completion chunks into one final message."""

from __future__ import annotations

from collections.abc import Iterable
from dataclasses import dataclass, field
from typing import Any


class IncompleteStreamError(RuntimeError):
    """Raised when the stream ends before any finish_reason arrives."""


@dataclass
class ToolCall:
    """A tool call assembled from indexed delta fragments."""

    id: str = ""
    name: str = ""
    arguments: str = ""


@dataclass
class Message:
    """The assistant message rebuilt from the first choice of a stream."""

    content: str = ""
    tool_calls: list[ToolCall] = field(default_factory=list)
    finish_reason: str | None = None


def accumulate(chunks: Iterable[dict[str, Any]]) -> Message:
    """Merge the deltas of choice 0 across ``chunks`` into a Message."""
    message = Message()
    calls: dict[int, ToolCall] = {}
    for chunk in chunks:
        for choice in chunk.get("choices", []):
            if choice.get("index", 0) != 0:
                continue
            delta = choice.get("delta", {})
            message.content += delta.get("content") or ""
            for part in delta.get("tool_calls") or []:
                call = calls.setdefault(part["index"], ToolCall())
                call.id = part.get("id")
                function = part.get("function") or {}
                call.name += function.get("name") or ""
                call.arguments += function.get("arguments") or ""
            if choice.get("finish_reason"):
                message.finish_reason = choice["finish_reason"]
    if message.finish_reason is None:
        raise IncompleteStreamError("stream ended without a finish_reason")
    message.tool_calls = [calls[index] for index in sorted(calls)]
    return message
