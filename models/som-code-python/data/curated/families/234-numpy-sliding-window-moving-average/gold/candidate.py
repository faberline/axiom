"""Trailing moving averages with NumPy sliding windows."""

from __future__ import annotations

import numpy as np
from numpy.lib.stride_tricks import sliding_window_view
from numpy.typing import ArrayLike, NDArray


def moving_average(
    values: ArrayLike, window: int, *, pad: bool = False
) -> NDArray[np.float64]:
    """Mean of every full window; with ``pad`` the first window-1 slots are NaN."""
    data = np.asarray(values, dtype=np.float64)
    if data.ndim != 1:
        raise ValueError("values must be 1-D")
    if not 1 <= window <= data.size:
        raise ValueError(f"window must be between 1 and {data.size}")
    means = sliding_window_view(data, window).mean(axis=1)
    if pad:
        return np.concatenate([np.full(window - 1, np.nan), means])
    return means
