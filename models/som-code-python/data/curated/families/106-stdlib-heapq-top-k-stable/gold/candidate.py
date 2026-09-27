"""Select the top scoring entries with a bounded heap and stable tie-breaking."""

import heapq
from collections.abc import Iterable
from dataclasses import dataclass


@dataclass(frozen=True, slots=True)
class Entry:
    """A named score."""

    name: str
    score: float


def top_k(entries: Iterable[Entry], k: int) -> list[Entry]:
    """Return the k highest scores, preferring earlier entries among ties."""
    if k < 0:
        raise ValueError("k must not be negative")
    if k == 0:
        return []
    heap: list[tuple[float, int, Entry]] = []
    for index, entry in enumerate(entries):
        item = (entry.score, -index, entry)
        if len(heap) < k:
            heapq.heappush(heap, item)
        elif item > heap[0]:
            heapq.heapreplace(heap, item)
    return [entry for _, _, entry in sorted(heap, reverse=True)]
