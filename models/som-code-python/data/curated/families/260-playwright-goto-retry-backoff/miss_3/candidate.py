"""Retry Playwright page.goto on navigation timeouts with exponential backoff."""

from __future__ import annotations

import time
from collections.abc import Callable
from typing import Any


class NavigationTimeoutError(Exception):
    """Raised by the page when a navigation times out."""


def goto_with_retry(
    page: Any,
    url: str,
    *,
    attempts: int = 3,
    base_delay: float = 0.5,
    sleep: Callable[[float], None] = time.sleep,
) -> Any:
    """Navigate, retrying only timeouts; the last timeout propagates."""
    if attempts < 1:
        raise ValueError("attempts must be at least 1")
    for attempt in range(1, attempts):
        try:
            return page.goto(url, wait_until="domcontentloaded")
        except Exception:
            sleep(base_delay * 2 ** (attempt - 1))
    return page.goto(url, wait_until="domcontentloaded")
