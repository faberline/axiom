"""A scikit-learn compatible transformer clipping columns to fitted quantiles."""

from __future__ import annotations

from typing import Self

import numpy as np
from numpy.typing import ArrayLike, NDArray
from sklearn.base import BaseEstimator, TransformerMixin
from sklearn.utils.validation import check_array, check_is_fitted


class QuantileClipper(TransformerMixin, BaseEstimator):  # type: ignore[misc]
    """Clip each feature to quantiles learned on the training data."""

    lower_: NDArray[np.float64]
    upper_: NDArray[np.float64]
    n_features_in_: int

    def __init__(self, lower: float = 0.01, upper: float = 0.99) -> None:
        self.lower = lower
        self.upper = upper

    def fit(self, x: ArrayLike, y: object = None) -> Self:
        """Learn per-column bounds; hyperparameters are validated here."""
        del y
        if not 0 <= self.lower < self.upper <= 1:
            raise ValueError("require 0 <= lower < upper <= 1")
        arr = check_array(x, dtype=np.float64)
        self.lower_ = np.quantile(arr, self.lower, axis=0)
        self.upper_ = np.quantile(arr, self.upper, axis=0)
        self.n_features_in_ = arr.shape[1]
        return self

    def transform(self, x: ArrayLike) -> NDArray[np.float64]:
        """Return a clipped copy of ``x``."""
        check_is_fitted(self)
        arr = check_array(x, dtype=np.float64)
        if arr.shape[1] != self.n_features_in_:
            raise ValueError(
                f"expected {self.n_features_in_} features, got {arr.shape[1]}"
            )
        clipped: NDArray[np.float64] = np.clip(arr, self.lower_, self.upper_, out=arr)
        return clipped
