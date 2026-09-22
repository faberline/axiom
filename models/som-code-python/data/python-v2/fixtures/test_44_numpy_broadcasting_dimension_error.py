import numpy as np
import pytest
import candidate


def test_gold_per_sample_normalization():
    rng = np.random.default_rng(42)
    X = rng.normal(loc=5.0, scale=3.0, size=(4, 5))
    normalizer = candidate.SampleMatrixNormalizer()
    out = normalizer.normalize_samples(X)
    assert out.shape == (4, 5)
    row_means = np.mean(out, axis=1)
    row_stds = np.std(out, axis=1)
    assert np.allclose(row_means, 0.0, atol=1e-5)
    assert np.allclose(row_stds, 1.0, atol=1e-3)


def test_default_epsilon_scale_fidelity():
    X = np.array([
        [1.0, 3.0, 5.0, 7.0],
        [2.0, 4.0, 6.0, 8.0],
    ], dtype=np.float64)
    normalizer = candidate.SampleMatrixNormalizer()
    out = normalizer.normalize_samples(X)
    row_stds = np.std(out, axis=1)
    assert np.all(np.abs(row_stds - 1.0) < 0.05)


def test_rejects_non_2d_array():
    normalizer = candidate.SampleMatrixNormalizer()
    with pytest.raises(ValueError, match="2D array"):
        normalizer.normalize_samples(np.array([1.0, 2.0, 3.0]))
    with pytest.raises(ValueError, match="2D array"):
        normalizer.normalize_samples(np.ones((2, 3, 4)))


def test_boundary_two_column_matrix_accepted():
    X = np.array([
        [10.0, 20.0],
        [30.0, 50.0],
        [0.0, 100.0],
    ], dtype=np.float64)
    normalizer = candidate.SampleMatrixNormalizer()
    out = normalizer.normalize_samples(X)
    assert out.shape == (3, 2)
    assert np.allclose(np.mean(out, axis=1), 0.0, atol=1e-5)
    assert np.allclose(np.std(out, axis=1), 1.0, atol=1e-3)


def test_normalizes_rows_not_columns():
    X = np.array([
        [1.0, 2.0, 3.0, 4.0],
        [10.0, 20.0, 30.0, 40.0],
        [100.0, 200.0, 300.0, 400.0],
    ], dtype=np.float64)
    normalizer = candidate.SampleMatrixNormalizer()
    out = normalizer.normalize_samples(X)
    assert np.allclose(np.mean(out, axis=1), 0.0, atol=1e-5)
    assert np.allclose(np.std(out, axis=1), 1.0, atol=1e-3)
    col_stds = np.std(out, axis=0)
    assert not np.allclose(col_stds, 1.0, atol=1e-2)


def test_square_and_rectangular_matrix_broadcasting():
    X_rect = np.arange(28, dtype=np.float64).reshape(4, 7)
    normalizer = candidate.SampleMatrixNormalizer()
    out = normalizer.normalize_samples(X_rect)
    assert out.shape == (4, 7)
    assert np.allclose(np.mean(out, axis=1), 0.0, atol=1e-5)

    X_sq = np.arange(16, dtype=np.float64).reshape(4, 4)
    out_sq = normalizer.normalize_samples(X_sq)
    assert out_sq.shape == (4, 4)
    assert np.allclose(np.mean(out_sq, axis=1), 0.0, atol=1e-5)


def test_optional_feature_centering():
    X = np.array([
        [10.0, 5.0, 2.0],
        [1.0, 8.0, 4.0],
        [7.0, 3.0, 9.0],
    ], dtype=np.float64)
    normalizer = candidate.SampleMatrixNormalizer(center_features=True)
    out = normalizer.normalize_samples(X)
    assert out.shape == (3, 3)
    assert np.allclose(np.mean(out, axis=1), 0.0, atol=1e-5)
    assert np.allclose(np.std(out, axis=1), 1.0, atol=1e-3)


def test_parameter_validations():
    with pytest.raises(ValueError, match="epsilon"):
        candidate.SampleMatrixNormalizer(epsilon=-1.0)
    with pytest.raises(ValueError, match="epsilon"):
        candidate.SampleMatrixNormalizer(epsilon=0.0)
    normalizer = candidate.SampleMatrixNormalizer()
    with pytest.raises(TypeError, match="numpy ndarray"):
        normalizer.normalize_samples([[1, 2], [3, 4]])
    with pytest.raises(ValueError, match="columns"):
        normalizer.normalize_samples(np.array([[1.0], [2.0]]))
