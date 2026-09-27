"""Split iterables into fixed-size chunks and sliding windows lazily."""

from collections import deque
from collections.abc import Iterable, Iterator
from itertools import islice


def chunked[T](
    items: Iterable[T], size: int, *, drop_partial: bool = True
) -> Iterator[tuple[T, ...]]:
    """Yield consecutive tuples of size items, optionally dropping a short tail."""
    if size < 1:
        raise ValueError("size must be at least 1")
    iterator = iter(items)
    while batch := tuple(islice(iterator, size)):
        if drop_partial and len(batch) < size:
            return
        yield batch


def sliding[T](items: Iterable[T], width: int) -> Iterator[tuple[T, ...]]:
    """Yield every full-width window over items, advancing one item at a time."""
    if width < 1:
        raise ValueError("width must be at least 1")
    window: deque[T] = deque(maxlen=width)
    for item in items:
        window.append(item)
        if len(window) == width:
            yield tuple(window)
