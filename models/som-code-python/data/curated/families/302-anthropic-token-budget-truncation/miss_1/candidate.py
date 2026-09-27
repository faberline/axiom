"""Trim a Claude conversation from the front until it fits a token budget."""

from __future__ import annotations

from typing import Any, Protocol

MODEL = "claude-sonnet-5"


class TokenCounter(Protocol):
    """The subset of client.messages used to count input tokens."""

    def count_tokens(self, **kwargs: Any) -> Any:
        """Return an object whose input_tokens is the request's size."""


class BudgetError(ValueError):
    """The conversation cannot be made to fit the budget."""


def _is_tool_result(message: dict[str, Any]) -> bool:
    content = message["content"]
    return isinstance(content, list) and any(
        block.get("type") == "tool_result" for block in content
    )


def _can_start(message: dict[str, Any]) -> bool:
    return message["role"] == "user" and not _is_tool_result(message)


def fit_history(
    counter: TokenCounter,
    system: str,
    messages: list[dict[str, Any]],
    budget: int,
    reserve: int = 256,
) -> list[dict[str, Any]]:
    """Return the longest suffix of messages that fits budget minus reserve.

    The suffix always starts with a plain user turn, so a tool_result is
    never separated from the tool_use that it answers.
    """
    if reserve < 0 or budget <= reserve:
        raise BudgetError("budget must exceed a non-negative reserve")
    if not messages or messages[-1]["role"] != "user":
        raise BudgetError("the conversation must end with a user turn")
    limit = budget - reserve
    start = 0
    while True:
        window = messages[start:]
        used = counter.count_tokens(model=MODEL, system=system, messages=window)
        if used.input_tokens < limit:
            return list(window)
        start += 1
        while start < len(messages) and not _can_start(messages[start]):
            start += 1
        if start >= len(messages):
            raise BudgetError(f"the last turn alone needs more than {limit} tokens")
