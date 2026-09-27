import warnings

import numpy as np
import pytest

from candidate import standardize


def test_columns_use_sample_standard_deviation():
    result = standardize([[1, 10], [2, 20], [3, 30]])
    np.testing.assert_allclose(result, [[-1, -1], [0, 0], [1, 1]])


def test_nan_cells_are_ignored_and_preserved():
    result = standardize(np.array([[1.0, 0.0], [np.nan, 1.0], [3.0, 2.0]]))
    assert np.isnan(result[1, 0])
    np.testing.assert_allclose(result[[0, 2], 0], [-(0.5**0.5), 0.5**0.5])


def test_constant_column_becomes_zero_without_warnings():
    with warnings.catch_warnings():
        warnings.simplefilter("error")
        result = standardize([[5.0, 1.0], [5.0, 2.0], [5.0, 3.0]])
    np.testing.assert_array_equal(result[:, 0], [0.0, 0.0, 0.0])


def test_one_dimensional_input_is_rejected():
    with pytest.raises(ValueError, match="2-D"):
        standardize([1.0, 2.0, 3.0])


def test_input_array_is_not_modified():
    data = np.array([[1.0, 4.0], [3.0, 8.0]])
    standardize(data)
    np.testing.assert_array_equal(data, [[1.0, 4.0], [3.0, 8.0]])
