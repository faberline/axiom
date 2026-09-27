"""Reproducible train/test index splits with a local NumPy Generator."""

from __future__ import annotations

import numpy as np
from numpy.typing import NDArray


def train_test_indices(
    n: int, test_fraction: float, *, seed: int
) -> tuple[NDArray[np.int64], NDArray[np.int64]]:
    """Return sorted (train, test) indices; both sides get at least one sample."""
    if n < 2:
        raise ValueError("need at least 2 samples")
    if not 0 < test_fraction < 1:
        raise ValueError("test_fraction must be strictly between 0 and 1")
    np.random.seed(seed)
    order = np.random.permutation(n)
    n_test = min(n - 1, max(1, round(n * test_fraction)))
    return np.sort(order[n_test:]), np.sort(order[:n_test])
