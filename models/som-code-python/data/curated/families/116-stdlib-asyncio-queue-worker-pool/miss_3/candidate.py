"""Fan work out to asyncio consumers through a bounded queue closed by sentinels."""

import asyncio
from collections.abc import Awaitable, Callable, Iterable


async def run_pipeline[T, R](
    items: Iterable[T],
    handle: Callable[[T], Awaitable[R]],
    *,
    workers: int = 3,
    maxsize: int = 10,
) -> list[R]:
    """Process items with a fixed pool of consumers and return results in order."""
    if workers < 1:
        raise ValueError("workers must be at least 1")
    queue: asyncio.Queue[tuple[int, T] | None] = asyncio.Queue(maxsize=maxsize)
    results: dict[int, R] = {}
    errors: dict[int, Exception] = {}

    async def consume() -> None:
        while (job := await queue.get()) is not None:
            index, item = job
            try:
                results[index] = await handle(item)
            except Exception as exc:  # pylint: disable=broad-exception-caught
                errors[index] = exc

    tasks = [asyncio.create_task(consume()) for _ in range(workers)]
    for job in enumerate(items):
        await queue.put(job)
    for _ in range(workers):
        await queue.put(None)
    await asyncio.gather(*tasks)
    if errors:
        raise ExceptionGroup("pipeline failed", [errors[i] for i in sorted(errors)])
    return list(results.values())
