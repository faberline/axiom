import asyncio
import threading
import time
import pytest
from candidate import OffloadedComputeEngine


def test_compute_hash_and_call_count():
    async def _test():
        engine = OffloadedComputeEngine(max_workers=2)
        try:
            digest = await engine.compute_hash(b"hello world", iterations=1000)
            assert isinstance(digest, str)
            assert len(digest) == 64
            assert engine.call_count == 1
        finally:
            engine.shutdown(wait=False)

    asyncio.run(_test())


def test_event_loop_not_blocked_during_execution():
    async def _test():
        engine = OffloadedComputeEngine(max_workers=2)
        main_thread = threading.get_ident()
        worker_threads = []

        def blocking_fn():
            worker_threads.append(threading.get_ident())
            time.sleep(0.08)
            return 42

        ticks = 0

        async def ticker():
            nonlocal ticks
            for _ in range(10):
                await asyncio.sleep(0.005)
                ticks += 1

        try:
            ticker_task = asyncio.create_task(ticker())
            res = await engine.run_blocking(blocking_fn)
            await ticker_task

            assert res == 42
            assert len(worker_threads) == 1
            assert worker_threads[0] != main_thread, "Execution ran synchronously on main event loop thread"
            assert ticks >= 1, "Event loop was completely blocked"
        finally:
            engine.shutdown(wait=False)

    asyncio.run(_test())


def test_managed_executor_used_without_spawning_unbounded_executors():
    async def _test():
        engine = OffloadedComputeEngine(max_workers=2)
        submit_calls = 0
        orig_submit = engine.executor.submit

        def counting_submit(*args, **kwargs):
            nonlocal submit_calls
            submit_calls += 1
            return orig_submit(*args, **kwargs)

        engine.executor.submit = counting_submit

        try:
            for _ in range(4):
                await engine.run_blocking(lambda: 1)
            assert submit_calls == 4, "Engine did not reuse its managed executor"
        finally:
            engine.shutdown(wait=False)

    asyncio.run(_test())


def test_call_count_lock_synchronization():
    async def _test():
        engine = OffloadedComputeEngine(max_workers=2)
        lock_entered = 0

        class TrackedLock(asyncio.Lock):
            async def __aenter__(self):
                nonlocal lock_entered
                lock_entered += 1
                return await super().__aenter__()

        engine._lock = TrackedLock()

        try:
            await engine.run_blocking(lambda: "val")
            assert lock_entered == 1, "call_count was incremented without acquiring self._lock"
        finally:
            engine.shutdown(wait=False)

    asyncio.run(_test())


def test_worker_exception_is_propagated():
    async def _test():
        engine = OffloadedComputeEngine(max_workers=2)

        def failing_fn():
            raise KeyError("missing_key")

        try:
            with pytest.raises(KeyError, match="missing_key"):
                await engine.run_blocking(failing_fn)
        finally:
            engine.shutdown(wait=False)

    asyncio.run(_test())
