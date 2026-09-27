"""Collect absolute, deduplicated outbound links from an HTML page with lxml."""

from __future__ import annotations

from urllib.parse import urldefrag, urlsplit

from lxml import html

ALLOWED_SCHEMES = frozenset({"http", "https"})


class PageError(Exception):
    """Raised when the page cannot be parsed."""


def extract_links(document: str, base_url: str) -> list[str]:
    """Return absolute http(s) links in first-seen order without fragments."""
    if not document.strip():
        raise PageError("empty document")
    tree = html.document_fromstring(document)
    if not isinstance(tree, html.HtmlElement):
        raise PageError("not an HTML document")
    tree.make_links_absolute(base_url, resolve_base_href=True)
    seen: set[str] = set()
    links: list[str] = []
    for element in tree.iter("a"):
        href = element.get("href")
        rel = (element.get("rel") or "").lower().split()
        if not href or "nofollow" in rel:
            continue
        url = href
        if urlsplit(url).scheme not in ALLOWED_SCHEMES or url in seen:
            continue
        seen.add(url)
        links.append(url)
    return links
