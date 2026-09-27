"""A LangChain-style JSON output parser with a bounded fix-and-retry loop."""

from __future__ import annotations

import json
import re
from collections.abc import Callable
from typing import Any

FENCE = re.compile(r"```(?:json)?\s*(.*?)```", re.DOTALL)


class OutputParserException(ValueError):  # noqa: N818
    """Raised when model output cannot be parsed into the schema."""

    def __init__(self, message: str, llm_output: str) -> None:
        super().__init__(message)
        self.llm_output = llm_output


class JsonOutputParser:
    """Parse a JSON object and check required keys and their types."""

    def __init__(self, schema: dict[str, type]) -> None:
        self.schema = dict(schema)

    def format_instructions(self) -> str:
        """Describe the expected object for the prompt."""
        fields = ", ".join(f'"{k}": {t.__name__}' for k, t in self.schema.items())
        return f"Return only a JSON object with {{{fields}}}."

    def parse(self, text: str) -> dict[str, Any]:
        """Return the validated object or raise OutputParserException."""
        match = FENCE.search(text)
        body = match.group(1) if match else text
        try:
            data = json.loads(body.strip())
        except json.JSONDecodeError as exc:
            raise OutputParserException(f"invalid JSON: {exc.msg}", text) from exc
        if not isinstance(data, dict):
            raise OutputParserException("expected a JSON object", text)
        for key, kind in self.schema.items():
            value = data.get(key)
            if not isinstance(value, kind):
                raise OutputParserException(f"{key!r} must be {kind.__name__}", text)
        return data


def parse_with_retry(
    llm: Callable[[str], str],
    parser: JsonOutputParser,
    prompt: str,
    max_retries: int = 2,
) -> dict[str, Any]:
    """Ask again with the error and bad output until parsing succeeds."""
    request = f"{prompt}\n{parser.format_instructions()}"
    output = llm(request)
    attempt = 0
    while True:
        try:
            return parser.parse(output)
        except OutputParserException as exc:
            if attempt >= max_retries:
                raise
            attempt += 1
            output = llm(
                f"{request}\nYour previous answer:\n{exc.llm_output}\n"
                f"Error: {exc}\nFix it."
            )
