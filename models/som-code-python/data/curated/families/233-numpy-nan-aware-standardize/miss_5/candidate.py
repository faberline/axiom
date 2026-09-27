"""Standardize feature columns with NumPy, tolerating NaN and constant columns."""

from __future__ import annotations

import numpy as np
from numpy.typing import ArrayLike, NDArray


def standardize(x: ArrayLike) -> NDArray[np.float64]:
    """Return per-column z-scores using the sample std; NaN cells stay NaN."""
    data = np.asarray(x, dtype=np.float64)
    if data.ndim != 2:
        raise ValueError(f"expected a 2-D array, got {data.ndim}-D")
    mean = np.nanmean(data)
    std = np.nanstd(data, axis=0, ddof=1)
    safe = np.where(std > 0, std, 1.0)
    return (data - mean) / safe
