"""Collect every page of a JSON list API by following Link headers."""

from __future__ import annotations

from collections.abc import Mapping
from typing import Any

import requests

MAX_PAGES = 50


class PaginationError(RuntimeError):
    """Raised when a listing is malformed or never ends."""


def fetch_all(
    session: requests.Session,
    url: str,
    *,
    params: Mapping[str, str] | None = None,
    max_pages: int = MAX_PAGES,
    timeout: float = 10.0,
) -> list[Any]:
    """Follow ``rel="next"`` links from ``url`` and concatenate each page."""
    items: list[Any] = []
    next_url = url
    query = params
    for _ in range(max_pages):
        resp = session.get(next_url, params=query, timeout=timeout)
        resp.raise_for_status()
        page = resp.json()
        items.extend(page)
        link = resp.links.get("next", {}).get("url")
        if link is None:
            return items
        next_url = link
        query = None
    raise PaginationError(f"listing did not end within {max_pages} pages")
