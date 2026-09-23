"""Dispatch OpenAI tool calls to registered functions and report every failure."""

import json
from collections.abc import Callable
from typing import Any

from pydantic import BaseModel, ValidationError


class ToolExecutionResult(BaseModel):
    """Outcome of one tool call, carrying its output or an error."""

    call_id: str
    name: str
    status: str
    output: Any = None
    error: str | None = None


class OpenAIToolDispatcher:
    """Registry mapping tool names to functions and optional argument schemas."""

    def __init__(self) -> None:
        self._registry: dict[
            str, tuple[Callable[..., Any], type[BaseModel] | None]
        ] = {}

    def register_tool(
        self,
        name: str,
        func: Callable[..., Any],
        schema: type[BaseModel] | None = None,
    ) -> None:
        """Register func under name, validating arguments with schema when given."""
        self._registry[name] = (func, schema)

    def execute_tool_call(self, tool_call: dict[str, Any]) -> ToolExecutionResult:
        """Run one tool call and return its result; never raises."""
        call_id = tool_call.get("id", "unknown")
        function_meta = tool_call.get("function", {})
        tool_name = function_meta.get("name")
        raw_args = function_meta.get("arguments", "")

        if tool_name not in self._registry:
            return ToolExecutionResult(
                call_id=call_id,
                name=tool_name or "unknown",
                status="error",
                error=f"Unrecognized tool: {tool_name}",
            )

        func, schema = self._registry[tool_name]

        try:
            parsed_args = json.loads(raw_args)
            if not isinstance(parsed_args, dict):
                return ToolExecutionResult(
                    call_id=call_id,
                    name=tool_name,
                    status="error",
                    error="Tool arguments must be a JSON object",
                )
        except json.JSONDecodeError as exc:
            return ToolExecutionResult(
                call_id=call_id,
                name=tool_name,
                status="error",
                error=f"Malformed JSON arguments: {exc.msg}",
            )

        if schema is not None:
            try:
                validated_model = schema.model_validate(parsed_args)
                kwargs = validated_model.model_dump()
            except ValidationError as val_err:
                return ToolExecutionResult(
                    call_id=call_id,
                    name=tool_name,
                    status="error",
                    error=f"Argument validation failed: {val_err!s}",
                )
        else:
            kwargs = parsed_args

        try:
            result = func(**kwargs)
            return ToolExecutionResult(
                call_id=call_id,
                name=tool_name,
                status="success",
                output=result,
            )
        # Any tool failure is reported back to the model instead of raised.
        except Exception as exc:  # pylint: disable=broad-exception-caught
            return ToolExecutionResult(
                call_id=call_id,
                name=tool_name,
                status="error",
                error=f"Tool execution failed: {exc!s}",
            )
