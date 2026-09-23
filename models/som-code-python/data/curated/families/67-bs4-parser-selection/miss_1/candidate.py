"""Structured markup parser with explicit parser features and CDATA support."""

from bs4 import BeautifulSoup, CData


class StructuredMarkupParser:
    """Parse HTML or XML with an explicitly chosen parser and keep CDATA content."""

    def __init__(self, mode: str = "xml") -> None:
        if mode not in ("xml", "html"):
            raise ValueError("mode must be either 'xml' or 'html'")
        self.mode = mode
        self.parser_feature = "lxml" if mode == "xml" else "lxml"

    def parse(self, markup: str) -> BeautifulSoup:
        """Create a BeautifulSoup instance with the configured parser feature."""
        if not markup or not markup.strip():
            raise ValueError("markup cannot be empty")
        return BeautifulSoup(markup, features=self.parser_feature)

    def extract_tag_values(self, markup: str, tag_name: str) -> list[str]:
        """Extract text or CDATA values for a specified tag, honoring casing rules."""
        if not tag_name or not tag_name.strip():
            raise ValueError("tag_name cannot be empty")
        soup = self.parse(markup)
        results: list[str] = []
        for tag in soup.find_all(tag_name):
            text_parts = []
            for child in tag.children:
                if isinstance(child, CData):
                    text_parts.append(str(child))
                elif string := getattr(child, "string", None):
                    text_parts.append(str(string))
            results.append("".join(text_parts).strip())
        return results
