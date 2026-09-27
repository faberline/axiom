"""Stream an aiohttp download to disk under a hard size cap."""

from __future__ import annotations

from pathlib import Path

import aiohttp

CHUNK_BYTES = 64 * 1024


class TooLargeError(Exception):
    """The response body is larger than the allowed size."""

    def __init__(self, limit: int) -> None:
        super().__init__(f"response exceeds {limit} bytes")
        self.limit = limit


async def download(
    session: aiohttp.ClientSession, url: str, dest: Path, *, max_bytes: int
) -> int:
    """Stream ``url`` into ``dest`` and return the number of bytes written."""
    async with session.get(url) as resp:
        resp.raise_for_status()
        if resp.content_length is not None and resp.content_length > max_bytes:
            raise TooLargeError(max_bytes)
        total = 0
        try:
            with dest.open("wb") as handle:
                async for chunk in resp.content.iter_chunked(CHUNK_BYTES):
                    total += len(chunk)
                    if total > max_bytes:
                        raise TooLargeError(max_bytes)
                    handle.write(chunk)
        except TooLargeError:
            dest.unlink(missing_ok=True)
            raise
    return total
