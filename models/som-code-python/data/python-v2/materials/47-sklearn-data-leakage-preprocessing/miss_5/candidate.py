"""Leak-free dataset splitter and standardizer preventing data leakage."""

from __future__ import annotations

from typing import Any
import numpy as np


class StandardScaler:
    """Feature standardizer that scales to zero mean and unit variance."""

    def __init__(self) -> None:
        self.mean_: np.ndarray | None = None
        self.var_: np.ndarray | None = None
        self.scale_: np.ndarray | None = None

    def fit(self, X: np.ndarray) -> StandardScaler:
        arr = np.asarray(X, dtype=np.float64)
        self.mean_ = arr.mean(axis=0)
        self.var_ = arr.var(axis=0)
        scale = np.sqrt(self.var_)
        self.scale_ = np.where(scale == 0.0, 1.0, scale)
        return self

    def transform(self, X: np.ndarray) -> np.ndarray:
        if self.mean_ is None or self.scale_ is None:
            raise ValueError("StandardScaler instance is not fitted yet.")
        arr = np.asarray(X, dtype=np.float64)
        return (arr - self.mean_) / self.scale_

    def fit_transform(self, X: np.ndarray) -> np.ndarray:
        return self.fit(X).transform(X)


class LeakFreeDataSplitter:
    """Orchestrates leak-free train/test splitting and standardization."""

    def __init__(self, test_size: float = 0.2, random_state: int = 42) -> None:
        if not (0.0 < test_size < 1.0):
            raise ValueError(f"test_size must be strictly between 0.0 and 1.0, got {test_size}")
        self.test_size = test_size
        self.random_state = random_state

    def split_and_scale(
        self,
        X: np.ndarray,
        y: np.ndarray,
    ) -> tuple[np.ndarray, np.ndarray, np.ndarray, np.ndarray, Any]:
        X_arr = np.asarray(X, dtype=np.float64)
        y_arr = np.asarray(y)

        if len(X_arr) != len(y_arr):
            raise ValueError(f"X and y lengths must match: got len(X)={len(X_arr)}, len(y)={len(y_arr)}")
        if len(X_arr) < 10:
            raise ValueError(f"Dataset must contain at least 10 samples, got {len(X_arr)}")

        n_samples = len(X_arr)
        n_test = int(round(n_samples * self.test_size))
        n_test = max(1, min(n_test, n_samples - 1))

        rng = np.random.RandomState(self.random_state)
        shuffled_indices = rng.permutation(n_samples)
        test_indices = shuffled_indices[:n_test]
        train_indices = shuffled_indices[n_test:]

        scaler = StandardScaler()
        X_scaled = scaler.fit_transform(X_arr)
        X_train_scaled = X_scaled[train_indices]
        X_test_scaled = X_scaled[test_indices]
        y_train = y_arr[train_indices]
        y_test = y_arr[test_indices]

        return X_train_scaled, X_test_scaled, y_train, y_test, scaler
