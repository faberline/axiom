"""Build a sitemaps.org XML document with lxml SubElement."""

from __future__ import annotations

from dataclasses import dataclass
from datetime import date
from typing import Any

from lxml import etree

SITEMAP_NS = "http://www.sitemaps.org/schemas/sitemap/0.9"
MAX_URLS = 50_000
NSMAP: dict[Any, str] = {None: SITEMAP_NS}


@dataclass(frozen=True)
class Page:
    """One URL entry."""

    loc: str
    lastmod: date | None = None
    priority: float | None = None


def build_sitemap(pages: list[Page]) -> bytes:
    """Serialise pages as a UTF-8 sitemap with an XML declaration."""
    if len(pages) > MAX_URLS:
        raise ValueError(f"a sitemap holds at most {MAX_URLS} URLs")
    urlset = etree.Element(f"{{{SITEMAP_NS}}}urlset", nsmap=NSMAP)
    for page in pages:
        if not page.loc.startswith(("http://", "https://")):
            raise ValueError(f"loc must be absolute: {page.loc!r}")
        url = etree.SubElement(urlset, f"{{{SITEMAP_NS}}}url")
        etree.SubElement(url, f"{{{SITEMAP_NS}}}loc").text = page.loc
        if page.lastmod is not None:
            lastmod = etree.SubElement(url, f"{{{SITEMAP_NS}}}lastmod")
            lastmod.text = page.lastmod.isoformat()
        if page.priority is not None:
            if not 0.0 < page.priority < 1.0:
                raise ValueError("priority must be between 0.0 and 1.0")
            priority = etree.SubElement(url, f"{{{SITEMAP_NS}}}priority")
            priority.text = f"{page.priority:.1f}"
    return etree.tostring(urlset, xml_declaration=True, encoding="UTF-8")
