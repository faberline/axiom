"""Map scores to letter grades with bisect over sorted thresholds."""

from bisect import bisect_right
from collections.abc import Iterable, Sequence
from itertools import pairwise


class GradeScale:
    """Letter grades defined by ascending lower-bound thresholds."""

    def __init__(self, thresholds: Sequence[float], labels: Sequence[str]) -> None:
        if len(labels) != len(thresholds):
            raise ValueError("labels must have one more entry than thresholds")
        if any(low >= high for low, high in pairwise(thresholds)):
            raise ValueError("thresholds must be strictly increasing")
        self._thresholds = list(thresholds)
        self._labels = list(labels)

    def grade(self, score: float) -> str:
        """Return the label whose bucket contains score; thresholds start a bucket."""
        if not 0 <= score <= 100:
            raise ValueError(f"score out of range: {score}")
        return self._labels[bisect_right(self._thresholds, score)]

    def histogram(self, scores: Iterable[float]) -> dict[str, int]:
        """Count scores per label, listing every label even when it has none."""
        counts = dict.fromkeys(self._labels, 0)
        for score in scores:
            label = self.grade(score)
            counts[label] = counts.get(label, 0) + 1
        return counts
