"""Extract Atom feed entries with namespace-aware XPath in lxml."""

from __future__ import annotations

from dataclasses import dataclass

from lxml import etree

NS = {"atom": "http://www.w3.org/2005/Atom"}
LINK_XPATH = "string(atom:link[@rel='alternate' or not(@rel)]/@href)"


class FeedError(Exception):
    """Raised when the document is not a usable Atom feed."""


@dataclass(frozen=True)
class Entry:
    """One feed entry."""

    title: str
    link: str
    updated: str


def parse_feed(data: bytes) -> list[Entry]:
    """Return the feed's entries in document order."""
    parser = etree.XMLParser()
    try:
        root = etree.fromstring(data, parser)
    except etree.XMLSyntaxError as exc:
        raise FeedError(f"invalid XML: {exc}") from exc
    if root.tag != f"{{{NS['atom']}}}feed":
        raise FeedError("not an Atom feed")
    entries: list[Entry] = []
    for node in root.findall("atom:entry", namespaces=NS):
        title = (node.findtext("atom:title", default="", namespaces=NS)).strip()
        link = str(node.xpath(LINK_XPATH, namespaces=NS))
        updated = node.findtext("atom:updated", default="", namespaces=NS)
        if not title or not link:
            raise FeedError("entry without title or link")
        entries.append(Entry(title, link, updated))
    return entries
