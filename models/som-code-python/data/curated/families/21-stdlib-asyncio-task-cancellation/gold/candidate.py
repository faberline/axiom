"""Run jobs that record their outcome and clean up even when cancelled."""

import asyncio
import contextlib
from collections.abc import Callable, Coroutine
from typing import Any


class ResilientJobRunner:
    """Track running jobs by id and cancel them on request."""

    def __init__(self) -> None:
        self.tasks: dict[str, asyncio.Task[Any]] = {}
        self.statuses: dict[str, str] = {}

    async def run_job(
        self,
        job_id: str,
        work_coro_fn: Callable[[], Coroutine[Any, Any, Any]],
        cleanup_coro_fn: Callable[[], Coroutine[Any, Any, Any]] | None = None,
    ) -> Any:
        """Run a job, record its status, and shield its cleanup from cancellation."""
        current_task = asyncio.current_task()
        if current_task is not None:
            self.tasks[job_id] = current_task
        self.statuses[job_id] = "running"
        try:
            res = await work_coro_fn()
            self.statuses[job_id] = "completed"
            return res
        except asyncio.CancelledError:
            self.statuses[job_id] = "cancelled"
            if cleanup_coro_fn is not None:
                await asyncio.shield(cleanup_coro_fn())
            raise
        except Exception:
            self.statuses[job_id] = "failed"
            if cleanup_coro_fn is not None:
                await asyncio.shield(cleanup_coro_fn())
            raise
        finally:
            self.tasks.pop(job_id, None)

    async def cancel_job(self, job_id: str, wait_timeout: float = 0.5) -> bool:
        """Cancel a running job and report whether it finished within the timeout."""
        task = self.tasks.get(job_id)
        if task is None or task.done():
            return False
        task.cancel()
        with contextlib.suppress(TimeoutError, asyncio.CancelledError):
            await asyncio.wait_for(asyncio.shield(task), timeout=wait_timeout)
        return task.done()
