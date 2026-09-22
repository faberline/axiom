from typing import Any, Callable, Optional
import json
from pydantic import BaseModel, ValidationError


class ToolExecutionResult(BaseModel):
    call_id: str
    name: str
    status: str
    output: Any = None
    error: Optional[str] = None


class OpenAIToolDispatcher:
    def __init__(self) -> None:
        self._registry: dict[str, tuple[Callable[..., Any], Optional[type[BaseModel]]]] = {}

    def register_tool(
        self,
        name: str,
        func: Callable[..., Any],
        schema: Optional[type[BaseModel]] = None,
    ) -> None:
        self._registry[name] = (func, schema)

    def execute_tool_call(self, tool_call: dict[str, Any]) -> ToolExecutionResult:
        call_id = tool_call.get("id", "unknown")
        function_meta = tool_call.get("function", {})
        tool_name = function_meta.get("name")
        raw_args = function_meta.get("arguments", "{}")

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
                validated_model = schema.model_validate_json(parsed_args)
                kwargs = validated_model.model_dump()
            except ValidationError as val_err:
                return ToolExecutionResult(
                    call_id=call_id,
                    name=tool_name,
                    status="error",
                    error=f"Argument validation failed: {str(val_err)}",
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
        except Exception as exc:
            return ToolExecutionResult(
                call_id=call_id,
                name=tool_name,
                status="error",
                error=f"Tool execution failed: {str(exc)}",
            )
