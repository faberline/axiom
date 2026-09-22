"""Oracle test suite for Family 47: Leak-Free Data Splitting & Preprocessing."""

import numpy as np
import pytest
from candidate import LeakFreeDataSplitter, StandardScaler


@pytest.fixture
def synthetic_data() -> tuple[np.ndarray, np.ndarray]:
    rng = np.random.RandomState(42)
    # Generate 100 samples with 4 features with non-zero mean shift
    X = rng.randn(100, 4) * 2.5 + np.array([10.0, -5.0, 3.0, 7.0])
    y = rng.randint(0, 2, size=100)
    return X, y


def test_gold_split_and_scale_proportions_and_shapes(synthetic_data):
    X, y = synthetic_data
    splitter = LeakFreeDataSplitter(test_size=0.2, random_state=42)
    X_train_scaled, X_test_scaled, y_train, y_test, scaler = splitter.split_and_scale(X, y)

    assert len(X_train_scaled) == 80
    assert len(X_test_scaled) == 20
    assert len(y_train) == 80
    assert len(y_test) == 20

    # Training data should be standardized to zero mean and unit variance
    np.testing.assert_allclose(X_train_scaled.mean(axis=0), np.zeros(4), atol=1e-7)
    np.testing.assert_allclose(X_train_scaled.std(axis=0), np.ones(4), atol=1e-7)

    # Scaler must expose mean_ and scale_
    assert hasattr(scaler, "mean_")
    assert hasattr(scaler, "scale_")
    assert scaler.mean_ is not None


def test_default_test_size_proportion(synthetic_data):
    X, y = synthetic_data
    splitter = LeakFreeDataSplitter(random_state=42)
    # Default test_size must be 0.2, producing exactly 20 test samples out of 100
    assert splitter.test_size == 0.2, f"Expected default test_size=0.2, got {splitter.test_size}"
    X_train_scaled, X_test_scaled, _, _, _ = splitter.split_and_scale(X, y)
    assert len(X_test_scaled) == 20, f"Expected 20 test samples, got {len(X_test_scaled)}"


def test_mismatched_x_y_lengths_rejected(synthetic_data):
    X, y = synthetic_data
    splitter = LeakFreeDataSplitter(test_size=0.2, random_state=42)
    with pytest.raises(ValueError, match="lengths must match"):
        splitter.split_and_scale(X[:50], y[:40])


def test_boundary_test_sizes_rejected():
    with pytest.raises(ValueError, match="test_size must be strictly between 0.0 and 1.0"):
        LeakFreeDataSplitter(test_size=0.0)
    with pytest.raises(ValueError, match="test_size must be strictly between 0.0 and 1.0"):
        LeakFreeDataSplitter(test_size=1.0)
    with pytest.raises(ValueError, match="test_size must be strictly between 0.0 and 1.0"):
        LeakFreeDataSplitter(test_size=-0.1)
    with pytest.raises(ValueError, match="test_size must be strictly between 0.0 and 1.0"):
        LeakFreeDataSplitter(test_size=1.5)


def test_scaler_fit_on_train_not_test(synthetic_data):
    X, y = synthetic_data
    splitter = LeakFreeDataSplitter(test_size=0.2, random_state=42)
    X_train_scaled, X_test_scaled, y_train, y_test, scaler = splitter.split_and_scale(X, y)

    # Recover the indices used by the splitter
    rng = np.random.RandomState(42)
    shuffled = rng.permutation(100)
    train_idx = shuffled[20:]
    test_idx = shuffled[:20]

    X_train = X[train_idx]
    X_test = X[test_idx]

    # Scaler mean must match X_train mean, not X_test mean
    np.testing.assert_allclose(scaler.mean_, X_train.mean(axis=0), atol=1e-7)
    assert not np.allclose(scaler.mean_, X_test.mean(axis=0), atol=1e-2), (
        "Scaler mean erroneously matched test split mean!"
    )


def test_scaler_has_no_data_leakage_from_full_dataset(synthetic_data):
    X, y = synthetic_data
    splitter = LeakFreeDataSplitter(test_size=0.2, random_state=42)
    X_train_scaled, X_test_scaled, y_train, y_test, scaler = splitter.split_and_scale(X, y)

    # Full dataset mean differs from train split mean
    full_mean = X.mean(axis=0)
    # Scaler mean must NOT match full dataset mean
    assert not np.allclose(scaler.mean_, full_mean, atol=1e-3), (
        "Data leakage detected: scaler mean equals full dataset mean!"
    )


def test_insufficient_samples_rejected():
    splitter = LeakFreeDataSplitter()
    X_tiny = np.zeros((8, 2))
    y_tiny = np.zeros(8)
    with pytest.raises(ValueError, match="at least 10 samples"):
        splitter.split_and_scale(X_tiny, y_tiny)
