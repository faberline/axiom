import asyncio

import pytest

from candidate import run_pipeline


def test_results_keep_input_order():
    async def handle(n):
        await asyncio.sleep(n * 0.01)
        return n * 10

    assert asyncio.run(run_pipeline([3, 1, 2], handle)) == [30, 10, 20]


def _peak(workers=None):
    state = {"now": 0, "peak": 0}

    async def handle(n):
        state["now"] += 1
        state["peak"] = max(state["peak"], state["now"])
        await asyncio.sleep(0.01)
        state["now"] -= 1
        return n

    kwargs = {} if workers is None else {"workers": workers}
    asyncio.run(run_pipeline(range(6), handle, **kwargs))
    return state["peak"]


def test_worker_count_bounds_concurrency():
    assert _peak(2) == 2
    assert _peak() == 3


def test_empty_input_and_bad_worker_count():
    async def handle(n):
        return n

    assert asyncio.run(run_pipeline([], handle)) == []
    with pytest.raises(ValueError, match="workers must be at least 1"):
        asyncio.run(run_pipeline([1], handle, workers=0))


def test_failures_are_grouped_and_do_not_stop_the_worker():
    processed = []

    async def handle(n):
        if n % 2:
            raise ValueError(f"odd {n}")
        processed.append(n)
        return n

    with pytest.raises(ExceptionGroup) as info:
        asyncio.run(run_pipeline([1, 2, 3, 4], handle, workers=1))
    assert [str(e) for e in info.value.exceptions] == ["odd 1", "odd 3"]
    assert processed == [2, 4]
