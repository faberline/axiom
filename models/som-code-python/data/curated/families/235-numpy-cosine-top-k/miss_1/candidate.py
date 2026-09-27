"""Rank corpus rows by cosine similarity to a query with NumPy."""

from __future__ import annotations

import numpy as np
from numpy.typing import ArrayLike, NDArray


def top_k_similar(
    query: ArrayLike, corpus: ArrayLike, k: int
) -> tuple[NDArray[np.intp], NDArray[np.float64]]:
    """Return (row indices, scores) of the k most similar rows, best first."""
    q = np.asarray(query, dtype=np.float64)
    m = np.asarray(corpus, dtype=np.float64)
    if m.ndim != 2 or q.shape != (m.shape[1],):
        raise ValueError("query length must match the corpus columns")
    if k < 1:
        raise ValueError("k must be positive")
    q_norm = np.linalg.norm(q)
    if q_norm == 0:
        raise ValueError("query vector is zero")
    norms = np.linalg.norm(m, axis=1)
    scores = np.zeros(m.shape[0])
    np.divide(m @ q, q_norm, out=scores, where=norms > 0)
    k = min(k, scores.size)
    top = np.argpartition(-scores, k - 1)[:k]
    order = top[np.argsort(-scores[top], kind="stable")]
    return order, scores[order]
