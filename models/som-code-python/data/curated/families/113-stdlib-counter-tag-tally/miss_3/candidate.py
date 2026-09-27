"""Tally tags with Counter: merge batches, forget exhausted tags, rank ties stably."""

from collections import Counter
from collections.abc import Iterable


def _normalize(tags: Iterable[str]) -> list[str]:
    """Lower-case and strip tags, dropping blank ones."""
    return [tag.strip().lower() for tag in tags if tag.strip()]


class TagTally:
    """Running tag counts."""

    def __init__(self) -> None:
        self._counts: Counter[str] = Counter()

    def add(self, tags: Iterable[str]) -> None:
        """Count each tag once per occurrence."""
        self._counts.update(_normalize(tags))

    def retract(self, tags: Iterable[str]) -> None:
        """Subtract tags and forget any whose count drops to zero or below."""
        self._counts.subtract(_normalize(tags))
        self._counts = +self._counts

    def count(self, tag: str) -> int:
        """Return the current count of one tag."""
        return self._counts[tag.strip().lower()]

    def top(self, n: int) -> list[tuple[str, int]]:
        """Return the n most frequent tags, ties ordered alphabetically."""
        if n < 0:
            raise ValueError("n must not be negative")
        ranked = sorted(self._counts.items(), key=lambda item: (-item[1], item[0]))
        return self._counts.most_common(n)

    def __len__(self) -> int:
        return len(self._counts)
