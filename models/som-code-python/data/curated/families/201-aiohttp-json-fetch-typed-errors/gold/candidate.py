"""Fetch a JSON object over HTTP with aiohttp and typed API errors."""

from __future__ import annotations

from typing import Any

import aiohttp

MAX_ERROR_CHARS = 200


class ApiError(Exception):
    """The API answered with an error status or an unusable body."""

    def __init__(self, status: int, message: str) -> None:
        super().__init__(f"{status}: {message}")
        self.status = status
        self.message = message


async def fetch_json(
    session: aiohttp.ClientSession,
    url: str,
    *,
    params: dict[str, str] | None = None,
) -> dict[str, Any]:
    """GET ``url`` and return its JSON object body or raise ApiError."""
    async with session.get(
        url, params=params, headers={"Accept": "application/json"}
    ) as resp:
        if resp.status >= 400:
            body = await resp.text()
            raise ApiError(resp.status, body[:MAX_ERROR_CHARS])
        if resp.content_type != "application/json":
            raise ApiError(resp.status, f"expected JSON, got {resp.content_type}")
        data = await resp.json()
        if not isinstance(data, dict):
            raise ApiError(resp.status, "expected a JSON object")
        return data
