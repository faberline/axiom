"""Sanitize user HTML down to an allowlist of tags and attributes."""

from __future__ import annotations

from urllib.parse import urlsplit

from bs4 import BeautifulSoup, Comment, Tag

ALLOWED_TAGS = frozenset({"p", "b", "i", "em", "strong", "a", "ul", "ol", "li", "br"})
ALLOWED_ATTRS = {"a": frozenset({"href", "title"})}
DROP_WITH_CONTENT = frozenset({"script", "style", "iframe", "object"})
SAFE_SCHEMES = frozenset({"http", "https", "mailto", ""})


def _safe_url(value: str) -> bool:
    scheme = urlsplit(value.strip()).scheme.lower()
    return scheme in SAFE_SCHEMES


def _clean_attrs(tag: Tag) -> None:
    allowed = ALLOWED_ATTRS.get(tag.name, frozenset())
    for name in list(tag.attrs):
        if name not in allowed:
            del tag.attrs[name]
    href = tag.get("href")
    if isinstance(href, str) and not _safe_url(href):
        del tag.attrs["href"]
    if tag.name == "a" and "href" in tag.attrs:
        tag.attrs["rel"] = "nofollow noopener"


def sanitize(html: str) -> str:
    """Return html with only allowlisted tags and attributes left."""
    soup = BeautifulSoup(html, "html.parser")
    for comment in soup.find_all(string=lambda text: isinstance(text, Comment)):
        comment.extract()
    for tag in soup.find_all(True):
        if not isinstance(tag, Tag) or tag.decomposed:
            continue
        if tag.name in DROP_WITH_CONTENT:
            tag.unwrap()
        elif tag.name not in ALLOWED_TAGS:
            tag.unwrap()
        else:
            _clean_attrs(tag)
    return str(soup)
