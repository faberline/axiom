"""Structured markup parser with explicit parser features and CDATA support."""
from typing import Any, List, Optional
from bs4 import BeautifulSoup, CData


class StructuredMarkupParser:
    """Parses HTML and XML documents with strict parser engine isolation and CDATA handling."""

    def __init__(self, mode: str = "xml") -> None:
        self.mode = mode
        self.parser_feature = "lxml-xml" if mode == "xml" else "lxml"

    def parse(self, markup: str) -> BeautifulSoup:
        """Create a BeautifulSoup instance with the explicitly configured parser feature."""
        if not markup or not markup.strip():
            raise ValueError("markup cannot be empty")
        return BeautifulSoup(markup, features=self.parser_feature)

    def extract_tag_values(self, markup: str, tag_name: str) -> List[str]:
        """Extract text or CDATA values for a specified tag, honoring casing rules."""
        if not tag_name or not tag_name.strip():
            raise ValueError("tag_name cannot be empty")
        soup = self.parse(markup)
        results: List[str] = []
        for tag in soup.find_all(tag_name):
            text_parts = []
            for child in tag.children:
                if isinstance(child, CData):
                    text_parts.append(str(child))
                elif getattr(child, "string", None):
                    text_parts.append(str(child.string))
            results.append("".join(text_parts).strip())
        return results
