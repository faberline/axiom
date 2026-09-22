"""Aiohttp response body consumption and connection release manager."""
from typing import Any, Dict, Optional
import aiohttp


class BadStatusError(Exception):
    """Raised when an HTTP response returns an unexpected status."""


class ResponseStreamConsumer:
    """Safely extracts headers and payloads, guaranteeing response connection release."""

    async def fetch_json_safe(
        self,
        session: aiohttp.ClientSession,
        url: str,
    ) -> Dict[str, Any]:
        """Fetch and decode JSON, ensuring response is released on error or status mismatch."""
        response = await session.get(url)
        try:
            if response.status != 200:
                await response.release()
                raise BadStatusError(f"Server returned status {response.status}")
            return await response.json()
        except Exception:
            await response.release()
            raise

    async def read_header_value(
        self,
        session: aiohttp.ClientSession,
        url: str,
        header_key: str,
    ) -> Optional[str]:
        """Read a single response header and immediately release the connection without reading body."""
        response = await session.get(url)
        try:
            return response.headers.get(header_key)
        finally:
            await response.release()
