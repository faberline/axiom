"""Bound slow async lookups with asyncio.wait_for and fall back to the next source."""

import asyncio
from collections.abc import Awaitable, Callable, Sequence


async def first_within[T](
    sources: Sequence[tuple[str, Callable[[], Awaitable[T]]]], timeout: float
) -> tuple[str, T]:
    """Return the name and value of the first source that answers in time."""
    if timeout < 0:
        raise ValueError("timeout must be positive")
    if not sources:
        raise ValueError("at least one source is required")
    for name, source in sources:
        try:
            return name, await asyncio.wait_for(source(), timeout)
        except TimeoutError:
            continue
    raise TimeoutError("every source timed out")
