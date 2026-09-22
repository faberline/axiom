import asyncio
from dataclasses import dataclass, field
from typing import Any, Awaitable, Callable, List


@dataclass
class BatchExecutionResult:
    successes: List[Any] = field(default_factory=list)
    failures: List[BaseException] = field(default_factory=list)
    cancelled_count: int = 0


class SafeBatchExecutor:
    async def execute_batch(
        self,
        tasks: list[Callable[[], Awaitable[Any]]],
        cancel_siblings_on_error: bool = False,
    ) -> BatchExecutionResult:
        if not tasks:
            return BatchExecutionResult()

        if not cancel_siblings_on_error:
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
        else:
            task_objs = [asyncio.create_task(task_fn()) for task_fn in tasks]
            done, pending = await asyncio.wait(
                task_objs,
                return_when=asyncio.FIRST_EXCEPTION,
            )
            has_error = any(
                not t.cancelled() and t.exception() is not None for t in done
            )
            if has_error and pending:
                for p in pending:
                    p.cancel()
                await asyncio.gather(*pending, return_exceptions=True)
            elif pending:
                done2, _ = await asyncio.wait(pending)
                done = done | done2

            successes = []
            failures = []
            cancelled_count = 0
            for t in task_objs:
                if t.cancelled():
                    cancelled_count += 1
                elif t.exception() is not None:
                    failures.append(t.exception())
                else:
                    successes.append(t.result())
            return BatchExecutionResult(
                successes=successes,
                failures=failures,
                cancelled_count=cancelled_count,
            )
