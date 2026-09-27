"""Count values into explicit bins with np.digitize and np.bincount."""

from __future__ import annotations

import numpy as np
from numpy.typing import ArrayLike, NDArray


def bucket_counts(values: ArrayLike, edges: ArrayLike) -> NDArray[np.intp]:
    """Count values per half-open bin; the last bin also holds the top edge."""
    v = np.asarray(values, dtype=np.float64)
    e = np.asarray(edges, dtype=np.float64)
    if e.ndim != 1 or e.size < 2 or np.any(np.diff(e) <= 0):
        raise ValueError("edges must be 1-D, strictly increasing, at least 2 long")
    idx = np.digitize(v, e) - 1
    inside = (idx >= 0) & (idx < e.size - 1)
    return np.bincount(idx[inside], minlength=e.size - 1)
