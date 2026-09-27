"""Validate XML documents against an XSD with lxml and report line numbers."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any

from lxml import etree


class SchemaLoadError(Exception):
    """Raised when the XSD itself is malformed."""


@dataclass(frozen=True)
class Violation:
    """One validation error."""

    line: int
    message: str


class OrderValidator:
    """Validate order documents against a fixed schema."""

    def __init__(self, xsd: bytes) -> None:
        try:
            self._schema = etree.XMLSchema(etree.fromstring(xsd, self._parser()))
        except (etree.XMLSyntaxError, etree.XMLSchemaParseError) as exc:
            raise SchemaLoadError(str(exc)) from exc

    @staticmethod
    def _parser() -> etree.XMLParser:
        return etree.XMLParser(resolve_entities=False, no_network=True)

    def violations(self, document: bytes) -> list[Violation]:
        """Return every problem in the document, sorted by line."""
        try:
            tree = etree.fromstring(document, self._parser())
        except etree.XMLSyntaxError as exc:
            return [Violation(exc.lineno or 0, f"syntax: {exc.msg}")]
        if self._schema.validate(tree):
            return []
        entries: list[Any] = [self._schema.error_log.last_error]  # type: ignore[call-overload]
        found = [Violation(err.line, err.message) for err in entries]
        return sorted(found, key=lambda v: v.line)

    def is_valid(self, document: bytes) -> bool:
        """Return True when the document has no violations."""
        return not self.violations(document)
