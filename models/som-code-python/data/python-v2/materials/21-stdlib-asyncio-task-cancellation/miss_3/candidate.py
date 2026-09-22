import asyncio
from typing import Any, Callable, Coroutine, Optional


class ResilientJobRunner:
    def __init__(self) -> None:
        self.tasks: dict[str, asyncio.Task] = {}
        self.statuses: dict[str, str] = {}

    async def run_job(
        self,
        job_id: str,
        work_coro_fn: Callable[[], Coroutine[Any, Any, Any]],
        cleanup_coro_fn: Optional[Callable[[], Coroutine[Any, Any, Any]]] = None,
    ) -> Any:
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
                await cleanup_coro_fn()
            raise
        except Exception:
            self.statuses[job_id] = "failed"
            if cleanup_coro_fn is not None:
                await asyncio.shield(cleanup_coro_fn())
            raise
        finally:
            self.tasks.pop(job_id, None)

    async def cancel_job(self, job_id: str, wait_timeout: float = 0.5) -> bool:
        task = self.tasks.get(job_id)
        if task is None or task.done():
            return False
        task.cancel()
        try:
            await asyncio.wait_for(asyncio.shield(task), timeout=wait_timeout)
        except (asyncio.CancelledError, asyncio.TimeoutError):
            pass
        return task.done()
