import asyncio

import pytest

from candidate import first_within


def test_slow_source_is_cancelled_and_the_next_one_answers():
    state = {"cancelled": False, "seen": None}

    async def primary():
        try:
            await asyncio.sleep(1)
        except asyncio.CancelledError:
            state["cancelled"] = True
            raise
        return "fresh"

    async def cache():
        state["seen"] = state["cancelled"]
        return "stale"

    result = asyncio.run(first_within([("db", primary), ("cache", cache)], 0.05))
    assert result == ("cache", "stale")
    assert state["seen"] is True


def test_fast_source_wins():
    async def primary():
        return 7

    async def cache():
        raise AssertionError("fallback must not run")

    assert asyncio.run(first_within([("db", primary), ("cache", cache)], 1)) == ("db", 7)


def test_real_errors_propagate_instead_of_falling_back():
    async def primary():
        raise RuntimeError("db down")

    async def cache():
        return "stale"

    with pytest.raises(RuntimeError, match="db down"):
        asyncio.run(first_within([("db", primary), ("cache", cache)], 1))


def test_all_slow_and_bad_arguments():
    async def slow():
        await asyncio.sleep(1)

    with pytest.raises(TimeoutError, match="every source timed out"):
        asyncio.run(first_within([("a", slow), ("b", slow)], 0.02))
    with pytest.raises(ValueError, match="timeout must be positive"):
        asyncio.run(first_within([("a", slow)], 0))
    with pytest.raises(ValueError, match="at least one source"):
        asyncio.run(first_within([], 1))
