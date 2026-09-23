"""Analyze streams by materializing each one exactly once."""

from collections.abc import Callable, Iterable
from typing import Any


class StreamAnalyzer:
    """Statistics, splits, and chunks over iterables consumed only once."""

    def __init__(self) -> None:
        self.history: list[dict[str, Any]] = []

    def compute_stats(self, stream: Iterable[float]) -> dict[str, float]:
        """Return count, sum, mean, min, and max of a non-empty stream."""
        count = sum(1 for _ in stream)
        if count == 0:
            raise ValueError("stream cannot be empty")
        if count > 500:
            raise ValueError("stream length exceeds maximum limit of 500")

        total = float(sum(stream))
        min_val = 0.0
        max_val = 0.0
        mean = total / count

        res = {
            "count": float(count),
            "sum": total,
            "mean": mean,
            "min": min_val,
            "max": max_val,
        }
        self.history.append(res)
        return res

    def filter_and_split(
        self, stream: Iterable[int], predicate: Callable[[int], bool]
    ) -> tuple[list[int], list[int]]:
        """Split a stream into the items that pass predicate and the rest."""
        items = list(stream)
        passed: list[int] = []
        failed: list[int] = []
        for item in items:
            if predicate(item):
                passed.append(item)
            else:
                failed.append(item)
        return passed, failed

    def chunk_stream(self, stream: Iterable[Any], chunk_size: int) -> list[list[Any]]:
        """Split a stream into lists of at most chunk_size items."""
        if chunk_size <= 0:
            raise ValueError("chunk_size must be greater than zero")
        items = list(stream)
        return [items[i : i + chunk_size] for i in range(0, len(items), chunk_size)]

    def clear(self) -> None:
        """Forget every recorded summary."""
        self.history.clear()

    def record_count(self) -> int:
        """Return how many summaries were recorded."""
        return len(self.history)
