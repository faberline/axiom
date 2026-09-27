"""Cache JSON GET responses and revalidate them with ETags."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any

import requests


@dataclass(frozen=True)
class _Entry:
    etag: str
    body: Any


class EtagCache:
    """Serve unchanged resources from memory after a 304 revalidation."""

    def __init__(self, session: requests.Session, *, timeout: float = 10.0) -> None:
        self._session = session
        self._timeout = timeout
        self._entries: dict[str, _Entry] = {}
        self.revalidated = 0

    def get_json(self, url: str) -> Any:
        """Return the JSON body of ``url``, reusing the cached copy on 304."""
        entry = self._entries.get(url)
        headers = {"If-None-Match": entry.etag} if entry else {}
        resp = self._session.get(url, headers=headers, timeout=self._timeout)
        if resp.status_code == 304 and entry is not None:
            self.revalidated += 1
            return entry.body
        resp.raise_for_status()
        body = resp.json()
        etag = resp.headers.get("ETag")
        if etag:
            self._entries[url] = _Entry(etag, body)
        return body
