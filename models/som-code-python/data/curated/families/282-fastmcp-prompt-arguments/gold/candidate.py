"""FastMCP-style prompts whose declared arguments render into messages."""

from __future__ import annotations

import string
from dataclasses import dataclass

ROLES = ("user", "assistant")


class PromptError(Exception):
    """Raised for badly declared prompts and bad render arguments."""


@dataclass(frozen=True)
class PromptArgument:
    """One prompt argument; optional ones fall back to their default."""

    name: str
    required: bool = True
    default: str = ""


@dataclass(frozen=True)
class Message:
    """A rendered prompt message."""

    role: str
    content: str


class Prompt:
    """A str.format template checked against its declared arguments."""

    def __init__(
        self,
        name: str,
        template: str,
        arguments: list[PromptArgument],
        role: str = "user",
    ) -> None:
        if role not in ROLES:
            raise PromptError(f"role must be one of {ROLES}")
        declared = {argument.name for argument in arguments}
        used = {
            field_name
            for _, field_name, _, _ in string.Formatter().parse(template)
            if field_name is not None
        }
        undeclared = sorted(used - declared)
        if undeclared:
            raise PromptError(f"undeclared arguments: {', '.join(undeclared)}")
        self.name = name
        self.template = template
        self.arguments = list(arguments)
        self.role = role

    def describe(self) -> dict[str, object]:
        """Return the prompt listing entry sent to clients."""
        return {
            "name": self.name,
            "arguments": [
                {"name": argument.name, "required": argument.required}
                for argument in self.arguments
            ],
        }

    def render(self, values: dict[str, str]) -> list[Message]:
        """Validate values, apply defaults and return the rendered message."""
        unknown = sorted(set(values) - {argument.name for argument in self.arguments})
        if unknown:
            raise PromptError(f"unknown arguments: {', '.join(unknown)}")
        missing = [
            argument.name
            for argument in self.arguments
            if argument.required and argument.name not in values
        ]
        if missing:
            raise PromptError(f"missing required arguments: {', '.join(missing)}")
        filled = {
            argument.name: values.get(argument.name, argument.default)
            for argument in self.arguments
        }
        return [Message(self.role, self.template.format_map(filled))]
