"""Cross-validate with GroupKFold so no group spans train and test folds."""

from __future__ import annotations

from dataclasses import dataclass

import numpy as np
from numpy.typing import ArrayLike
from sklearn.base import BaseEstimator
from sklearn.model_selection import GroupKFold, cross_val_score


@dataclass(frozen=True)
class CVResult:
    """Per-fold scores with their mean and sample standard deviation."""

    scores: tuple[float, ...]
    mean: float
    std: float


def grouped_cv_score(
    model: BaseEstimator,
    x: ArrayLike,
    y: ArrayLike,
    groups: ArrayLike,
    *,
    n_splits: int = 5,
) -> CVResult:
    """Score ``model`` by accuracy with GroupKFold over ``groups``."""
    group_arr = np.asarray(groups)
    if group_arr.shape[0] != np.asarray(y).shape[0]:
        raise ValueError("groups must align with y")
    n_groups = np.unique(group_arr).size
    if n_groups <= n_splits:
        raise ValueError(f"need at least {n_splits} groups, got {n_groups}")
    scores = cross_val_score(
        model,
        x,
        y,
        groups=group_arr,
        cv=GroupKFold(n_splits=n_splits),
        scoring="accuracy",
    )
    return CVResult(
        scores=tuple(float(s) for s in scores),
        mean=float(np.mean(scores)),
        std=float(np.std(scores, ddof=1)),
    )
