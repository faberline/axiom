"""Resilient aiohttp request fetcher handling disconnects and timeouts."""

import asyncio
from typing import Any

import aiohttp


class FetchError(Exception):
    """Raised when fetching fails after all retries are exhausted."""


class ResilientAiohttpFetcher:
    """Perform HTTP requests with a timeout, bounded retries, and safe cancellation."""

    def __init__(
        self,
        total_timeout: float = 5.0,
        connect_timeout: float = 2.0,
        max_retries: int = 2,
    ) -> None:
        if total_timeout <= 0 or connect_timeout <= 0:
            raise ValueError("Timeout values must be positive")
        if max_retries < 0:
            raise ValueError("max_retries cannot be negative")

        self.total_timeout = total_timeout
        self.connect_timeout = connect_timeout
        self.max_retries = max_retries
        self.timeout_config = aiohttp.ClientTimeout(
            total=total_timeout,
            connect=connect_timeout,
        )

    async def fetch_json(
        self,
        session: aiohttp.ClientSession,
        url: str,
    ) -> dict[str, Any]:
        """Fetch a JSON payload, retrying transient network and timeout errors.

        asyncio.CancelledError is a BaseException, so it is never retried.
        """
        last_error: Exception | None = None
        for attempt in range(self.max_retries + 1):
            try:
                async with session.get(url, timeout=None) as response:
                    if response.status != 200:
                        raise aiohttp.ClientResponseError(
                            request_info=response.request_info,
                            history=response.history,
                            status=response.status,
                            message=f"HTTP status {response.status}",
                        )
                    payload: dict[str, Any] = await response.json()
                    return payload
            except (TimeoutError, aiohttp.ClientError) as exc:
                last_error = exc
                if attempt == self.max_retries:
                    break
                await asyncio.sleep(0.001)

        raise FetchError(
            f"Failed to fetch {url} after {self.max_retries + 1} attempts"
        ) from last_error
