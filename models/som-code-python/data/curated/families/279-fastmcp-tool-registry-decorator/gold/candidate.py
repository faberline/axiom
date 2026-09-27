"""A FastMCP-style server whose tools register through a decorator."""

from __future__ import annotations

import inspect
from collections.abc import Callable
from dataclasses import dataclass
from typing import Any

ToolFn = Callable[..., Any]


class ToolError(Exception):
    """Raised for registration conflicts and failed or malformed tool calls."""


@dataclass(frozen=True)
class Tool:
    """A registered tool with its parameter names and required subset."""

    name: str
    description: str
    fn: ToolFn
    params: tuple[str, ...]
    required: tuple[str, ...]


class FastMCP:
    """Register tools with @mcp.tool() and dispatch calls by name."""

    def __init__(self, name: str) -> None:
        self.name = name
        self._tools: dict[str, Tool] = {}

    def tool(self, name: str | None = None) -> Callable[[ToolFn], ToolFn]:
        """Return a decorator that registers the function and returns it."""

        def register(fn: ToolFn) -> ToolFn:
            tool_name = name or fn.__name__
            if tool_name in self._tools:
                raise ToolError(f"tool {tool_name!r} is already registered")
            parameters = inspect.signature(fn).parameters.values()
            required = tuple(
                p.name for p in parameters if p.default is inspect.Parameter.empty
            )
            params = tuple(p.name for p in parameters)
            description = inspect.getdoc(fn) or ""
            self._tools[tool_name] = Tool(tool_name, description, fn, params, required)
            return fn

        return register

    def list_tools(self) -> list[Tool]:
        """Return registered tools sorted by name."""
        return sorted(self._tools.values(), key=lambda tool: tool.name)

    def call_tool(self, name: str, arguments: dict[str, Any]) -> Any:
        """Validate arguments, call the tool and wrap its failures."""
        tool = self._tools.get(name)
        if tool is None:
            raise ToolError(f"unknown tool {name!r}")
        unexpected = sorted(set(arguments) - set(tool.params))
        if unexpected:
            raise ToolError(f"unexpected arguments: {', '.join(unexpected)}")
        missing = [param for param in tool.required if param not in arguments]
        if missing:
            raise ToolError(f"missing arguments: {', '.join(missing)}")
        try:
            return tool.fn(**arguments)
        except (ArithmeticError, LookupError, TypeError, ValueError) as exc:
            raise ToolError(f"tool {name!r} failed: {exc}") from exc
