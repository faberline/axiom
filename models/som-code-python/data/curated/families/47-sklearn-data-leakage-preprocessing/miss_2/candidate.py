"""Split a dataset first, then fit the StandardScaler on the training rows only."""

import numpy as np
from sklearn.model_selection import train_test_split
from sklearn.preprocessing import StandardScaler

__all__ = ["LeakFreeDataSplitter", "StandardScaler"]


class LeakFreeDataSplitter:
    """Split features and targets, then standardize without test-set leakage."""

    def __init__(self, test_size: float = 0.2, random_state: int = 42) -> None:
        if not 0.0 < test_size < 1.0:
            raise ValueError(
                f"test_size must be strictly between 0.0 and 1.0, got {test_size}"
            )
        self.test_size = test_size
        self.random_state = random_state

    def split_and_scale(
        self,
        X: np.ndarray,
        y: np.ndarray,
    ) -> tuple[np.ndarray, np.ndarray, np.ndarray, np.ndarray, StandardScaler]:
        """Return scaled train and test features, their targets, and the scaler."""
        if len(X) < 10:
            raise ValueError(f"Dataset must contain at least 10 samples, got {len(X)}")

        X_train, X_test, y_train, y_test = train_test_split(
            np.asarray(X, dtype=np.float64),
            np.asarray(y),
            test_size=self.test_size,
            random_state=self.random_state,
        )
        scaler = StandardScaler()
        X_train_scaled = scaler.fit_transform(X_train)
        X_test_scaled = scaler.transform(X_test)
        return X_train_scaled, X_test_scaled, y_train, y_test, scaler
