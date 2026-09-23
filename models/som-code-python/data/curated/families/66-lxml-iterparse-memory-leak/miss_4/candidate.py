"""Memory-safe XML stream processor using lxml iterparse."""

from collections.abc import Iterator
from typing import Any, BinaryIO

from lxml import etree


class StreamXmlRecordExtractor:
    """Extracts records from large XML streams with constant memory overhead."""

    def __init__(self, target_tag: str) -> None:
        if not target_tag or not target_tag.strip():
            raise ValueError("target_tag cannot be empty")
        self.target_tag = target_tag.strip()
        self.last_root: Any | None = None

    def iter_records(self, source: BinaryIO) -> Iterator[dict[str, str]]:
        """Yield one dict per target element, clearing each so memory stays flat."""
        context = etree.iterparse(source, events=("end",))
        for _, elem in context:
            parent = elem.getparent()
            self.last_root = parent if parent is not None else context.root
            record = {child.tag: (child.text or "").strip() for child in elem}
            yield record
            elem.clear()
            if parent is not None:
                while elem.getprevious() is not None:
                    del parent[0]
        if context.root is not None:
            self.last_root = context.root
