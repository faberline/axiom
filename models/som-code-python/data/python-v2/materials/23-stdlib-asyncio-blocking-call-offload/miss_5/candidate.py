import asyncio
import concurrent.futures
import hashlib
from typing import Any, Callable, TypeVar

T = TypeVar("T")


class OffloadedComputeEngine:
    def __init__(self, max_workers: int = 4) -> None:
        self.executor = concurrent.futures.ThreadPoolExecutor(max_workers=max_workers)
        self._lock = asyncio.Lock()
        self.call_count: int = 0

    async def run_blocking(self, sync_fn: Callable[[], T]) -> T:
        async with self._lock:
            self.call_count += 1
        loop = asyncio.get_running_loop()
        try:
            return await loop.run_in_executor(self.executor, sync_fn)
        except Exception:
            return None

    async def compute_hash(self, data: bytes, iterations: int = 50_000) -> str:
        def _hash_worker() -> str:
            current = data
            for _ in range(iterations):
                current = hashlib.sha256(current).digest()
            return current.hex()

        return await self.run_blocking(_hash_worker)

    def shutdown(self, wait: bool = True) -> None:
        self.executor.shutdown(wait=wait)
