"""Offload blocking work to a thread pool so the event loop stays responsive."""

import asyncio
import concurrent.futures
import hashlib
from collections.abc import Callable


class OffloadedComputeEngine:
    """Run synchronous callables on a shared thread pool."""

    def __init__(self, max_workers: int = 4) -> None:
        self.executor = concurrent.futures.ThreadPoolExecutor(max_workers=max_workers)
        self._lock = asyncio.Lock()
        self.call_count: int = 0

    async def run_blocking[T](self, sync_fn: Callable[[], T]) -> T:
        """Run a callable on the pool and await its result."""
        self.call_count += 1
        loop = asyncio.get_running_loop()
        return await loop.run_in_executor(self.executor, sync_fn)

    async def compute_hash(self, data: bytes, iterations: int = 50_000) -> str:
        """Hash ``data`` repeatedly with SHA-256 off the event loop."""

        def _hash_worker() -> str:
            current = data
            for _ in range(iterations):
                current = hashlib.sha256(current).digest()
            return current.hex()

        return await self.run_blocking(_hash_worker)

    def shutdown(self, wait: bool = True) -> None:
        """Shut the thread pool down."""
        self.executor.shutdown(wait=wait)
