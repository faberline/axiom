"""A Playwright page.route handler that blocks heavy and third-party requests."""

from __future__ import annotations

from collections.abc import Iterable
from typing import Any
from urllib.parse import urlsplit

BLOCKED_TYPES = frozenset({"image", "font", "media"})


class RequestBlocker:
    """Abort blocked resource types and hosts outside the allow list."""

    def __init__(self, allowed_hosts: Iterable[str]) -> None:
        hosts = frozenset(host.lower() for host in allowed_hosts)
        self._allowed = hosts
        self.blocked: list[str] = []

    def _host_allowed(self, host: str) -> bool:
        return any(host == h or host.endswith("." + h) for h in self._allowed)

    def handle(self, route: Any) -> None:
        """Abort or continue one intercepted route."""
        request = route.request
        host = urlsplit(request.url).hostname or ""
        if request.resource_type in BLOCKED_TYPES or not self._host_allowed(host):
            self.blocked.append(request.url)
            route.abort()
            return
        route.continue_()

    def reset(self) -> None:
        """Forget the recorded blocked URLs."""
        self.blocked.clear()
