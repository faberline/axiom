"""Run a batch of coroutines and sort every outcome into successes and failures."""

import asyncio
from collections.abc import Callable, Coroutine
from dataclasses import dataclass, field
from typing import Any

type TaskFactory = Callable[[], Coroutine[Any, Any, Any]]


@dataclass
class BatchExecutionResult:
    """The results, exceptions, and cancellations of one batch."""

    successes: list[Any] = field(default_factory=list)
    failures: list[BaseException] = field(default_factory=list)
    cancelled_count: int = 0


class SafeBatchExecutor:
    """Run coroutine factories together without losing any exception."""

    async def execute_batch(
        self,
        tasks: list[TaskFactory],
        cancel_siblings_on_error: bool = False,
    ) -> BatchExecutionResult:
        """Run every task, optionally cancelling the rest after the first failure."""
        if not tasks:
            return BatchExecutionResult()

        if not cancel_siblings_on_error:
            return await self._gather_all(tasks)
        return await self._cancel_on_first_error(tasks)

    @staticmethod
    async def _gather_all(tasks: list[TaskFactory]) -> BatchExecutionResult:
        raw_results = await asyncio.gather(
            *[task_fn() for task_fn in tasks],
            return_exceptions=True,
        )
        successes: list[Any] = []
        failures: list[BaseException] = []
        cancelled_count = 0
        for item in raw_results:
            if isinstance(item, asyncio.CancelledError):
                cancelled_count += 1
            elif isinstance(item, BaseException):
                failures.append(item)
            else:
                successes.append(item)
        return BatchExecutionResult(
            successes=successes,
            failures=failures,
            cancelled_count=cancelled_count,
        )

    @staticmethod
    async def _cancel_on_first_error(tasks: list[TaskFactory]) -> BatchExecutionResult:
        task_objs = [asyncio.create_task(task_fn()) for task_fn in tasks]
        done, pending = await asyncio.wait(
            task_objs,
            return_when=asyncio.FIRST_EXCEPTION,
        )
        has_error = any(not t.cancelled() and t.exception() is not None for t in done)
        if has_error and pending:
            await asyncio.gather(*pending, return_exceptions=True)
        elif pending:
            await asyncio.wait(pending)

        successes: list[Any] = []
        failures: list[BaseException] = []
        cancelled_count = 0
        for t in task_objs:
            if t.cancelled():
                cancelled_count += 1
            elif (exc := t.exception()) is not None:
                failures.append(exc)
            else:
                successes.append(t.result())
        return BatchExecutionResult(
            successes=successes,
            failures=failures,
            cancelled_count=cancelled_count,
        )
