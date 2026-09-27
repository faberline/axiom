"""Derive an OpenAI function-tool definition from a Python signature."""

from __future__ import annotations

import inspect
from collections.abc import Callable
from typing import Any, get_type_hints

JSON_TYPES: dict[object, str] = {
    str: "string",
    int: "integer",
    float: "number",
    bool: "boolean",
}


class UnsupportedParameterError(TypeError):
    """Raised for parameters that cannot be expressed as a JSON schema field."""


def tool_schema(func: Callable[..., Any]) -> dict[str, Any]:
    """Return the ``tools`` entry describing ``func`` for chat completions."""
    hints = get_type_hints(func)
    properties: dict[str, Any] = {}
    required: list[str] = []
    for name, param in inspect.signature(func).parameters.items():
        if param.kind in (param.VAR_POSITIONAL, param.VAR_KEYWORD):
            raise UnsupportedParameterError(f"{name}: variadic parameters")
        json_type = JSON_TYPES.get(hints.get(name))
        properties[name] = {"type": json_type}
        if param.default is param.empty:
            required.append(name)
    return {
        "type": "function",
        "function": {
            "name": func.__name__,
            "description": inspect.getdoc(func) or "",
            "parameters": {
                "type": "object",
                "properties": properties,
                "required": required,
                "additionalProperties": False,
            },
        },
    }
