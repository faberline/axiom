"""Fixture for 179: async client code tested with AsyncMock."""

import asyncio
from unittest.mock import AsyncMock, call

import pytest

from candidate import fake_client, load_profiles


def tracking_client() -> tuple[AsyncMock, list[int]]:
    client = fake_client({})
    active = [0, 0]

    async def get(path: str) -> dict[str, object]:
        active[0] += 1
        active[1] = max(active[1], active[0])
        await asyncio.sleep(0)
        active[0] -= 1
        return {"path": path}

    client.get.side_effect = get
    return client, active


def test_loads_known_users_and_skips_missing() -> None:
    client = fake_client({"/users/1": {"name": "a"}, "/users/3": {"name": "c"}})
    result = asyncio.run(load_profiles(client, [1, 2, 3]))
    assert result == {1: {"name": "a"}, 3: {"name": "c"}}
    client.get.assert_has_awaits(
        [call("/users/1"), call("/users/2"), call("/users/3")], any_order=True
    )
    client.aclose.assert_awaited_once()


def test_client_is_closed_when_get_fails() -> None:
    client = fake_client({})
    client.get.side_effect = RuntimeError("boom")
    with pytest.raises(RuntimeError, match="boom"):
        asyncio.run(load_profiles(client, [1]))
    client.aclose.assert_awaited_once()


def test_concurrency_never_exceeds_the_limit() -> None:
    client, active = tracking_client()
    result = asyncio.run(load_profiles(client, range(6), limit=3))
    assert len(result) == 6
    assert active[1] == 3


def test_default_limit_is_two() -> None:
    client, active = tracking_client()
    asyncio.run(load_profiles(client, range(5)))
    assert active[1] == 2


def test_limit_must_be_positive() -> None:
    client = fake_client({})
    with pytest.raises(ValueError, match="limit must be at least 1"):
        asyncio.run(load_profiles(client, [1], limit=-1))
    client.get.assert_not_awaited()


def test_fake_client_is_spec_limited() -> None:
    client = fake_client({})
    with pytest.raises(AttributeError):
        _ = client.post
    with pytest.raises(LookupError, match="/users/9"):
        asyncio.run(client.get("/users/9"))
