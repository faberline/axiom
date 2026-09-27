import numpy as np
import pytest

from candidate import bucket_counts

EDGES = [0.0, 1.0, 2.0, 3.0]


def test_half_open_bins_with_closed_last_bin():
    counts = bucket_counts([0, 0.5, 1, 1.5, 2, 3, 3.0], EDGES)
    assert counts.tolist() == [2, 2, 3]


def test_out_of_range_and_nan_values_are_ignored():
    counts = bucket_counts([-1, 3.5, np.nan, 0.2], EDGES)
    assert counts.tolist() == [1, 0, 0]


def test_trailing_empty_bins_are_reported():
    assert bucket_counts([0.1], EDGES).tolist() == [1, 0, 0]
    assert bucket_counts([], EDGES).tolist() == [0, 0, 0]


def test_matches_numpy_histogram():
    rng = np.random.default_rng(0)
    values = rng.uniform(-1, 11, size=500).round(1)
    edges = np.array([0, 2.5, 5, 7.5, 10])
    expected, _ = np.histogram(values, edges)
    np.testing.assert_array_equal(bucket_counts(values, edges), expected)


def test_invalid_edges_are_rejected():
    for edges in ([0, 0, 1], [1], [[0, 1], [1, 2]], [2, 1]):
        with pytest.raises(ValueError, match="strictly increasing"):
            bucket_counts([0.5], edges)
