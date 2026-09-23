"""Run a pool of worker processes that survives a slow or stuck shutdown."""

import contextlib
import multiprocessing as mp
import queue
import time
from multiprocessing.queues import Queue
from typing import Any


def _worker_entry(task_queue: Queue[Any], result_queue: Queue[dict[str, Any]]) -> None:
    """Double each task until the stop sentinel arrives."""
    while True:
        try:
            item = task_queue.get(timeout=0.1)
        except (queue.Empty, TimeoutError):
            continue
        if item == "STOP_SENTINEL":
            break
        try:
            res = item * 2 if isinstance(item, (int, float, str)) else item
            result_queue.put({"success": True, "result": res})
        except (TypeError, ValueError) as exc:
            result_queue.put({"success": False, "error": str(exc)})


class ResilientWorkerPool:
    """Start, feed, drain, and stop a fixed set of worker processes."""

    def __init__(self, num_workers: int = 2) -> None:
        self.num_workers = num_workers
        self.workers: list[mp.Process] = []
        self.task_queue: Queue[Any] | None = None
        self.result_queue: Queue[dict[str, Any]] | None = None
        self._running = False

    def start(self) -> None:
        """Spawn the workers and their queues once."""
        if self._running:
            return
        self.task_queue = mp.Queue()
        self.result_queue = mp.Queue()
        self.workers = []
        for _ in range(self.num_workers):
            p = mp.Process(
                target=_worker_entry, args=(self.task_queue, self.result_queue)
            )
            p.start()
            self.workers.append(p)
        self._running = True

    def submit(self, item: Any) -> None:
        """Queue one task, refusing when the pool is not running."""
        if not self._running or self.task_queue is None:
            raise RuntimeError("Pool is not running")
        self.task_queue.put(item)

    def collect_results(self, expected_count: int, timeout: float = 1.0) -> list[Any]:
        """Wait for expected_count results within one shared deadline."""
        if not self._running or self.result_queue is None:
            raise RuntimeError("Pool is not running")
        results: list[Any] = []
        deadline = time.monotonic() + timeout
        for _ in range(expected_count):
            rem = max(0.01, deadline - time.monotonic())
            try:
                res = self.result_queue.get(timeout=rem)
                results.append(res)
            except (queue.Empty, TimeoutError) as exc:
                raise TimeoutError("Timeout waiting for worker results") from exc
        return results

    def shutdown(self, timeout: float = 1.0) -> None:
        """Stop every worker, terminating any that miss its share of timeout."""
        if not self._running:
            return
        for p in self.workers:
            p.terminate()
            p.join()
        for q in (self.task_queue, self.result_queue):
            if q is not None:
                with contextlib.suppress(ValueError, OSError):
                    q.close()
                    q.join_thread()
        self._running = False
