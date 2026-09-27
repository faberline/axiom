"""Aiohttp response body consumption and connection release manager."""

from typing import Any

import aiohttp


class BadStatusError(Exception):
    """Raised when an HTTP response returns an unexpected status."""


class ResponseStreamConsumer:
    """Extract headers and payloads, always releasing the response connection."""

    async def fetch_json_safe(
        self,
        session: aiohttp.ClientSession,
        url: str,
    ) -> dict[str, Any]:
        """Fetch and decode JSON, releasing the response on any error or bad status."""
        response = await session.get(url)
        try:
            if response.status != 200:
                await response.release()
                raise BadStatusError(f"Server returned status {response.status}")
            payload: dict[str, Any] = await response.json()
            return payload
        except Exception:
            await response.release()
            raise

    async def read_header_value(
        self,
        session: aiohttp.ClientSession,
        url: str,
        header_key: str,
    ) -> str | None:
        """Read one response header and release the connection without the body."""
        response = await session.get(url)
        return response.headers.get(header_key)
