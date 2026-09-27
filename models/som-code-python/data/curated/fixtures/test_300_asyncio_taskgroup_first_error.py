import asyncio

import pytest

from candidate import CheckFailedError, run_checks


def ok(value, delay=0.0):
    async def check():
        await asyncio.sleep(delay)
        return value

    return check


def fail(exc, delay=0.0):
    async def check():
        await asyncio.sleep(delay)
        raise exc

    return check


def test_results_are_keyed_in_input_order():
    out = asyncio.run(run_checks({"db": ok(1, 0.02), "cache": ok("up")}))
    assert out == {"db": 1, "cache": "up"}
    assert list(out) == ["db", "cache"]


def test_first_failure_is_raised_bare_and_siblings_cancelled():
    cancelled = []

    async def slow():
        try:
            await asyncio.sleep(1)
        except asyncio.CancelledError:
            cancelled.append("slow")
            raise

    with pytest.raises(CheckFailedError) as info:
        asyncio.run(run_checks({"slow": slow, "db": fail(OSError("refused"), 0.01)}))
    assert info.value.name == "db"
    assert isinstance(info.value.cause, OSError)
    assert info.value.__cause__ is None
    assert cancelled == ["slow"]


def test_simultaneous_failures_pick_smallest_name():
    checks = {"zeta": fail(ValueError("z")), "alpha": fail(OSError("a"))}
    with pytest.raises(CheckFailedError) as info:
        asyncio.run(run_checks(checks))
    assert info.value.name == "alpha"


def test_unexpected_errors_keep_the_group():
    with pytest.raises(ExceptionGroup) as info:
        asyncio.run(run_checks({"bug": fail(KeyError("k")), "ok": ok(1)}))
    assert isinstance(info.value.exceptions[0], KeyError)


def test_deadline_and_arguments():
    with pytest.raises(TimeoutError):
        asyncio.run(run_checks({"hang": ok(1, 1.0)}, deadline=0.05))
    with pytest.raises(ValueError):
        asyncio.run(run_checks({}))
    with pytest.raises(ValueError):
        asyncio.run(run_checks({"a": ok(1)}, deadline=0))
