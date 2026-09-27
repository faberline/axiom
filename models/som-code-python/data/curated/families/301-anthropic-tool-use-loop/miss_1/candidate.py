"""Drive a Claude Messages API tool-use loop against a duck-typed client."""

from __future__ import annotations

import json
from collections.abc import Callable, Mapping
from typing import Any, Protocol

Tool = Callable[..., Any]


class MessagesAPI(Protocol):
    """The subset of client.messages that the loop needs."""

    def create(self, **kwargs: Any) -> Any:
        """Send one Messages API request and return the response."""


class ToolLoopError(RuntimeError):
    """The model kept calling tools past the turn limit."""


def _run_tool(tools: Mapping[str, Tool], block: Any) -> dict[str, Any]:
    result: dict[str, Any] = {"type": "tool_result", "tool_use_id": block.id}
    tool = tools.get(block.name)
    if tool is None:
        result.update(content=f"unknown tool {block.name!r}", is_error=True)
        return result
    try:
        output = tool(**block.input)
    except (TypeError, ValueError, KeyError) as exc:
        result.update(content=f"{type(exc).__name__}: {exc}", is_error=True)
        return result
    result["content"] = output if isinstance(output, str) else json.dumps(output)
    return result


def run_tool_loop(
    messages_api: MessagesAPI,
    prompt: str,
    tools: Mapping[str, Tool],
    schemas: list[dict[str, Any]],
    max_turns: int = 5,
) -> str:
    """Ask the model, execute every tool_use block, and return the final text."""
    history: list[dict[str, Any]] = [{"role": "user", "content": prompt}]
    for _ in range(max_turns):
        response = messages_api.create(
            model="claude-sonnet-5", max_tokens=1024, tools=schemas, messages=history
        )
        if response.stop_reason != "tool_use":
            return "".join(b.text for b in response.content if b.type == "text")
        results = [
            _run_tool(tools, b) for b in response.content if b.type == "tool_use"
        ]
        history.append({"role": "user", "content": results})
    raise ToolLoopError(f"no final answer after {max_turns} turns")
