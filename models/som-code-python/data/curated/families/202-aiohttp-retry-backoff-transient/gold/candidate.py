"""Retry idempotent aiohttp GETs on transient failures with capped backoff."""

from __future__ import annotations

import asyncio
from collections.abc import Awaitable, Callable

import aiohttp

RETRY_STATUSES = frozenset({429, 502, 503, 504})
BASE_DELAY = 0.5
MAX_DELAY = 8.0


def _retry_after(value: str | None) -> float | None:
    if value is not None and value.isdigit():
        return float(value)
    return None


async def get_with_retry(
    session: aiohttp.ClientSession,
    url: str,
    *,
    attempts: int = 4,
    sleep: Callable[[float], Awaitable[None]] = asyncio.sleep,
) -> bytes:
    """GET ``url``, retrying transient statuses and connection errors."""
    if attempts < 1:
        raise ValueError("attempts must be at least 1")
    for attempt in range(attempts - 1):
        try:
            async with session.get(url) as resp:
                if resp.status not in RETRY_STATUSES:
                    resp.raise_for_status()
                    return await resp.read()
                delay = _retry_after(resp.headers.get("Retry-After"))
        except aiohttp.ClientConnectionError:
            delay = None
        if delay is None:
            delay = BASE_DELAY * 2**attempt
        await sleep(min(delay, MAX_DELAY))
    async with session.get(url) as resp:
        resp.raise_for_status()
        return await resp.read()
