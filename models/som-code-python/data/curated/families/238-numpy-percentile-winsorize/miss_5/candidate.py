"""Winsorize a 1-D sample at NaN-aware percentiles without mutating it."""

from __future__ import annotations

import numpy as np
from numpy.typing import ArrayLike, NDArray


def winsorize(
    values: ArrayLike, lower: float = 5.0, upper: float = 95.0
) -> NDArray[np.float64]:
    """Clip values to the [lower, upper] percentiles; NaN stays NaN."""
    if not 0 <= lower < upper <= 100:
        raise ValueError("percentiles must satisfy 0 <= lower < upper <= 100")
    arr = np.asarray(values, dtype=np.float64)
    if arr.ndim != 1:
        raise ValueError("values must be 1-D")
    lo, hi = np.nanpercentile(arr, [lower, upper])
    return np.clip(arr, lo, hi)
