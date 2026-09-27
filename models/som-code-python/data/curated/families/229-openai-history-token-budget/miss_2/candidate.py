"""Trim chat history to a token budget before sending it to OpenAI."""

from __future__ import annotations

from collections.abc import Callable

Message = dict[str, str]
MESSAGE_OVERHEAD = 4


class BudgetTooSmallError(ValueError):
    """Raised when system messages plus the newest turn exceed the budget."""


def trim_history(
    messages: list[Message], budget: int, count_tokens: Callable[[str], int]
) -> list[Message]:
    """Keep every system message and the newest contiguous turns that fit."""

    def cost(message: Message) -> int:
        return count_tokens(message["content"]) + MESSAGE_OVERHEAD

    system = [m for m in messages if m["role"] == "system"]
    turns = [m for m in messages if m["role"] != "system"]
    if not turns:
        raise ValueError("history has no conversation turns")
    used = sum(cost(m) for m in system) + cost(turns[-1])
    if used > budget:
        raise BudgetTooSmallError(f"need {used} tokens, budget is {budget}")
    kept = [turns[-1]]
    for message in reversed(turns[:-1]):
        extra = cost(message)
        if used + extra >= budget:
            break
        kept.append(message)
        used += extra
    kept.reverse()
    return system + kept
