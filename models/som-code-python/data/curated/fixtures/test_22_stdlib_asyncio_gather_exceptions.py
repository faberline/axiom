import asyncio
import pytest
from candidate import SafeBatchExecutor, BatchExecutionResult


def test_batch_all_successes():
    async def _test():
        executor = SafeBatchExecutor()

        async def task_1():
            await asyncio.sleep(0.01)
            return "val1"

        async def task_2():
            await asyncio.sleep(0.01)
            return "val2"

        res = await executor.execute_batch([task_1, task_2], cancel_siblings_on_error=False)
        assert res.successes == ["val1", "val2"]
        assert len(res.failures) == 0
        assert res.cancelled_count == 0

    asyncio.run(_test())


def test_batch_mixed_return_exceptions():
    async def _test():
        executor = SafeBatchExecutor()

        async def task_ok():
            await asyncio.sleep(0.01)
            return "ok"

        async def task_err():
            await asyncio.sleep(0.01)
            raise ValueError("task error")

        res = await executor.execute_batch([task_ok, task_err], cancel_siblings_on_error=False)
        assert len(res.successes) == 1
        assert res.successes[0] == "ok"
        assert len(res.failures) == 1
        assert isinstance(res.failures[0], ValueError)
        assert not any(isinstance(x, BaseException) for x in res.successes)

    asyncio.run(_test())


def test_batch_failures_not_dropped():
    async def _test():
        executor = SafeBatchExecutor()

        async def task_err():
            await asyncio.sleep(0.01)
            raise RuntimeError("boom")

        res = await executor.execute_batch([task_err], cancel_siblings_on_error=False)
        assert len(res.failures) == 1
        assert isinstance(res.failures[0], RuntimeError)

    asyncio.run(_test())


def test_fail_fast_cancels_siblings_and_counts_cancelled():
    async def _test():
        executor = SafeBatchExecutor()
        long_task_completed = False

        async def failing_task():
            await asyncio.sleep(0.01)
            raise RuntimeError("fast fail")

        async def long_task():
            nonlocal long_task_completed
            await asyncio.sleep(0.2)
            long_task_completed = True
            return "done"

        res = await executor.execute_batch(
            [failing_task, long_task],
            cancel_siblings_on_error=True,
        )

        assert len(res.failures) == 1
        assert isinstance(res.failures[0], RuntimeError)
        assert res.cancelled_count == 1
        assert long_task_completed is False

    asyncio.run(_test())


def test_fail_fast_does_not_count_cancellations_as_failures():
    async def _test():
        executor = SafeBatchExecutor()

        async def fail_task():
            await asyncio.sleep(0.01)
            raise ValueError("fail")

        async def slow_task():
            await asyncio.sleep(0.2)
            return 42

        res = await executor.execute_batch(
            [fail_task, slow_task],
            cancel_siblings_on_error=True,
        )

        for f in res.failures:
            assert not isinstance(f, asyncio.CancelledError)
        assert res.cancelled_count == 1

    asyncio.run(_test())
