"""Fetch many URLs concurrently with aiohttp under a concurrency limit."""

from __future__ import annotations

import asyncio
from collections.abc import Sequence
from dataclasses import dataclass

import aiohttp


@dataclass(frozen=True)
class FetchResult:
    """Outcome of one fetch: a status and body, or the error class name."""

    url: str
    status: int | None
    body: bytes | None
    error: str | None


async def fetch_all(
    session: aiohttp.ClientSession, urls: Sequence[str], *, limit: int = 5
) -> list[FetchResult]:
    """Fetch ``urls`` with at most ``limit`` in flight, keeping input order."""
    if limit < 1:
        raise ValueError("limit must be at least 1")
    semaphore = asyncio.Semaphore(limit)

    async def one(url: str) -> FetchResult:
        async with semaphore:
            async with session.get(url) as resp:
                return FetchResult(url, resp.status, await resp.read(), None)

    return list(await asyncio.gather(*(one(url) for url in urls)))
