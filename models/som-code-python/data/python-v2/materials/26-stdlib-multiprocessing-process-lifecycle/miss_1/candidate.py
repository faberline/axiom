import multiprocessing as mp
import queue
import time
from typing import Any, List, Optional


def _worker_entry(task_queue: mp.Queue, result_queue: mp.Queue) -> None:
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
        except Exception as exc:
            result_queue.put({"success": False, "error": str(exc)})


class ResilientWorkerPool:
    def __init__(self, num_workers: int = 2):
        self.num_workers = num_workers
        self.workers: List[mp.Process] = []
        self.task_queue: Optional[mp.Queue] = None
        self.result_queue: Optional[mp.Queue] = None
        self._running = False

    def start(self) -> None:
        if self._running:
            return
        self.task_queue = mp.Queue()
        self.result_queue = mp.Queue()
        self.workers = []
        for _ in range(self.num_workers):
            p = mp.Process(target=_worker_entry, args=(self.task_queue, self.result_queue))
            p.start()
            self.workers.append(p)
        self._running = True

    def submit(self, item: Any) -> None:
        if not self._running or self.task_queue is None:
            raise RuntimeError("Pool is not running")
        self.task_queue.put(item)

    def collect_results(self, expected_count: int, timeout: float = 1.0) -> List[Any]:
        if not self._running or self.result_queue is None:
            raise RuntimeError("Pool is not running")
        results = []
        deadline = time.monotonic() + timeout
        for _ in range(expected_count):
            rem = max(0.01, deadline - time.monotonic())
            try:
                res = self.result_queue.get(timeout=rem)
                results.append(res)
            except (queue.Empty, TimeoutError):
                raise TimeoutError("Timeout waiting for worker results")
        return results

    def shutdown(self, timeout: float = 1.0) -> None:
        if not self._running:
            return
        if self.task_queue is not None:
            for _ in self.workers:
                try:
                    self.task_queue.put("STOP_SENTINEL")
                except Exception:
                    pass
        per_worker_timeout = max(0.05, timeout / max(1, len(self.workers)))
        for p in self.workers:
            p.join(timeout=per_worker_timeout)
            if p.is_alive():
                p.terminate()
        for q in (self.task_queue, self.result_queue):
            if q is not None:
                try:
                    q.close()
                    q.join_thread()
                except Exception:
                    pass
        self._running = False
