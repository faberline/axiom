"""Bound how many coroutines run at once and keep counters of what they did."""

import asyncio
from collections.abc import Awaitable, Callable
from dataclasses import dataclass


@dataclass(frozen=True)
class LimiterStats:
    """A point-in-time snapshot of a limiter's counters."""

    max_concurrency: int
    peak_concurrency: int
    active_count: int
    completed_count: int
    failed_count: int


class ConcurrencyLimiter:
    """Run coroutine factories with at most ``max_concurrency`` in flight."""

    def __init__(self, max_concurrency: int = 1) -> None:
        if max_concurrency <= 0:
            raise ValueError("max_concurrency must be greater than 0")
        self.max_concurrency = max_concurrency
        self._semaphore = asyncio.Semaphore(max_concurrency)
        self._lock = asyncio.Lock()
        self._active = 0
        self._peak = 0
        self._completed = 0
        self._failed = 0

    async def execute[T](self, task_func: Callable[[], Awaitable[T]]) -> T:
        """Run one task under the limit, counting its success or failure."""
        async with self._semaphore:
            async with self._lock:
                self._active += 1
                self._peak = max(self._peak, self._active)
            try:
                result = await task_func()
                async with self._lock:
                    self._completed += 1
                return result
            except Exception:
                async with self._lock:
                    self._failed += 1
                raise
            finally:
                async with self._lock:
                    self._active -= 1

    async def run_batch[T](self, tasks: list[Callable[[], Awaitable[T]]]) -> list[T]:
        """Run every task under the limit and return results in input order."""
        results: list[T] = []
        async with asyncio.TaskGroup() as tg:
            task_objects = [tg.create_task(self.execute(task)) for task in tasks]
        for task_obj in task_objects:
            results.append(task_obj.result())
        return results

    def get_stats(self) -> LimiterStats:
        """Return a snapshot of the current counters."""
        return LimiterStats(
            max_concurrency=self.max_concurrency,
            peak_concurrency=self._peak,
            active_count=self._active,
            completed_count=self._completed,
            failed_count=self._failed,
        )

    def reset_state(self) -> None:
        """Clear every counter and replace the semaphore."""
        self._semaphore = asyncio.Semaphore(self.max_concurrency)
        self._active = 0
        self._peak = 0
        self._completed = 0
        self._failed = 0
