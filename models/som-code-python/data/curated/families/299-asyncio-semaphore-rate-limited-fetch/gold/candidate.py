"""Fetch many URLs concurrently with a bounded number in flight."""

from __future__ import annotations

import asyncio
from collections.abc import Awaitable, Callable, Iterable
from dataclasses import dataclass

Fetch = Callable[[str], Awaitable[bytes]]


@dataclass(frozen=True)
class Outcome:
    """The body or the error for one URL."""

    url: str
    body: bytes | None = None
    error: str | None = None


async def _one(
    fetch: Fetch, url: str, gate: asyncio.Semaphore, timeout: float
) -> Outcome:
    async with gate:
        try:
            body = await asyncio.wait_for(fetch(url), timeout)
        except TimeoutError:
            return Outcome(url, error="timeout")
        except OSError as exc:
            return Outcome(url, error=f"{type(exc).__name__}: {exc}")
    return Outcome(url, body=body)


async def fetch_all(
    fetch: Fetch, urls: Iterable[str], limit: int = 4, timeout: float = 5.0
) -> list[Outcome]:
    """Fetch every url with at most limit requests in flight, in input order."""
    if limit < 1:
        raise ValueError("limit must be at least 1")
    if timeout <= 0:
        raise ValueError("timeout must be positive")
    gate = asyncio.Semaphore(limit)
    unique = list(dict.fromkeys(urls))
    return list(
        await asyncio.gather(*(_one(fetch, url, gate, timeout) for url in unique))
    )
