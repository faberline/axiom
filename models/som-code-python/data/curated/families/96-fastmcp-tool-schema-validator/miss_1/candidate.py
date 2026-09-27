"""Generate strict tool schemas from function signatures and validate them."""

import inspect
from collections.abc import Callable
from typing import Any, ClassVar, get_args, get_origin, get_type_hints


class FastMCPToolSchemaValidator:
    """Build and check JSON tool schemas, optionally in strict mode."""

    TYPE_MAP: ClassVar[dict[type, str]] = {
        str: "string",
        int: "integer",
        float: "number",
        bool: "boolean",
    }

    def __init__(self, strict_mode: bool = True) -> None:
        self.strict_mode = strict_mode

    def _resolve_param_schema(self, py_type: Any) -> dict[str, Any]:
        """Map a Python type hint to its JSON schema, defaulting to string."""
        origin = get_origin(py_type)
        if origin is list:
            args = get_args(py_type)
            item_type = args[0] if args else str
            item_schema = self._resolve_param_schema(item_type)
            return {"type": "array", "items": item_schema}

        if py_type in self.TYPE_MAP:
            return {"type": self.TYPE_MAP[py_type]}

        return {"type": "string"}

    def generate_schema_for_function(self, func: Callable[..., Any]) -> dict[str, Any]:
        """Return a function-calling schema for func's signature and docstring."""
        sig = inspect.signature(func)
        type_hints = get_type_hints(func)

        properties: dict[str, Any] = {}
        required: list[str] = []

        for param_name, param in sig.parameters.items():
            if param_name in ("self", "cls"):
                continue

            param_type = type_hints.get(param_name, str)
            properties[param_name] = self._resolve_param_schema(param_type)

            if self.strict_mode or param.default is inspect.Parameter.empty:
                required.append(param_name)

        parameters_schema: dict[str, Any] = {
            "type": "object",
            "properties": properties,
            "required": required,
        }

        return {
            "type": "function",
            "function": {
                "name": func.__name__,
                "description": inspect.getdoc(func) or "",
                "parameters": parameters_schema,
                "strict": self.strict_mode,
            },
        }

    def validate_schema(self, schema: dict[str, Any]) -> tuple[bool, list[str]]:
        """Return whether schema is valid, with every problem found."""
        errors: list[str] = []
        func_schema = schema.get("function", {})
        params = func_schema.get("parameters", {})

        if params.get("type") != "object":
            errors.append("Top-level parameters must have type 'object'")

        if self.strict_mode and params.get("additionalProperties") is not False:
            errors.append("Strict schemas must set additionalProperties to false")

        props = params.get("properties", {})
        req = params.get("required", [])

        if self.strict_mode:
            missing_req = set(props.keys()) - set(req)
            if missing_req:
                errors.append(
                    "Strict mode requires all properties in 'required', missing: "
                    f"{missing_req}"
                )

        for p_name, p_schema in props.items():
            if "type" not in p_schema:
                errors.append(f"Property '{p_name}' missing explicit type")
            if p_schema.get("type") == "array" and "items" not in p_schema:
                errors.append(f"Array property '{p_name}' missing 'items' schema")

        return (len(errors) == 0, errors)
