import numpy as np


class SampleMatrixNormalizer:
    def __init__(self, epsilon: float = 1e-8, center_features: bool = False) -> None:
        if epsilon <= 0.0:
            raise ValueError("epsilon must be strictly positive")
        self.epsilon = float(epsilon)
        self.center_features = bool(center_features)

    def normalize_samples(self, X: np.ndarray) -> np.ndarray:
        if not isinstance(X, np.ndarray):
            raise TypeError("X must be a numpy ndarray")
        if X.ndim != 2:
            raise ValueError("X must be a 2D array")
        if X.shape[1] < 2:
            raise ValueError("X must have at least 2 feature columns")

        work = X.astype(np.float64)
        if self.center_features:
            work = work - np.mean(work, axis=0, keepdims=True)

        mean = np.mean(work, axis=1)
        std = np.std(work, axis=1)
        return (work - mean) / (std + self.epsilon)
