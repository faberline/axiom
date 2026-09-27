"""Parse an OpenAI structured-output chat response into a pydantic model."""

from __future__ import annotations

from typing import Any

from pydantic import BaseModel, ValidationError


class StructuredOutputError(Exception):
    """Base class for every way a structured response can be unusable."""


class RefusalError(StructuredOutputError):
    """The model refused; ``reason`` carries its refusal text."""

    def __init__(self, reason: str) -> None:
        super().__init__(reason)
        self.reason = reason


class TruncatedOutputError(StructuredOutputError):
    """The output stopped at the token limit, so its JSON is incomplete."""


class SchemaMismatchError(StructuredOutputError):
    """The content is not valid JSON for the requested schema."""


def parse_structured[T: BaseModel](response: dict[str, Any], model: type[T]) -> T:
    """Return the first choice's content validated as ``model``."""
    choice = response["choices"][0]
    message = choice["message"]
    if message.get("refusal"):
        raise RefusalError(message["refusal"])
    if choice.get("finish_reason") == "length":
        raise TruncatedOutputError("output stopped at the token limit")
    try:
        return model.model_validate_json(message.get("content") or "")
    except ValidationError as exc:
        raise SchemaMismatchError(str(exc)) from exc
