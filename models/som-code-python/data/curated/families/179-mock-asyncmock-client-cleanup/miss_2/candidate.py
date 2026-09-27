"""Test concurrent async client code against an AsyncMock client."""

import asyncio
from collections.abc import Iterable, Mapping
from typing import Protocol
from unittest.mock import AsyncMock

type Body = dict[str, object]


class Client(Protocol):
    """The slice of an HTTP client that load_profiles uses."""

    async def get(self, path: str) -> Body:
        """Return the JSON body at path or raise LookupError."""

    async def aclose(self) -> None:
        """Release the connection pool."""


def fake_client(responses: Mapping[str, Body]) -> AsyncMock:
    """An AsyncMock client whose get returns mapped bodies and raises LookupError."""

    async def get(path: str) -> Body:
        if path not in responses:
            raise LookupError(path)
        return responses[path]

    client = AsyncMock(spec_set=Client)
    client.get.side_effect = get
    return client


async def load_profiles(
    client: Client, user_ids: Iterable[int], limit: int = 2
) -> dict[int, Body]:
    """Fetch users at most limit at a time, skip missing ones, always close."""
    if limit < 1:
        raise ValueError("limit must be at least 1")
    gate = asyncio.Semaphore(limit)

    async def one(user_id: int) -> tuple[int, Body | None]:
        async with gate:
            try:
                return user_id, await client.get(f"/users/{user_id}")
            except LookupError:
                return user_id, None

    pairs = await asyncio.gather(*(one(user_id) for user_id in user_ids))
    await client.aclose()
    return {user_id: body for user_id, body in pairs if body is not None}
