import numpy as np
import pytest

from candidate import moving_average


def test_full_windows_only():
    result = moving_average([1, 2, 3, 4, 5, 6], 3)
    np.testing.assert_allclose(result, [2, 3, 4, 5])


def test_padding_keeps_input_length_with_leading_nan():
    result = moving_average([1, 2, 3, 4, 5, 6], 3, pad=True)
    assert result.shape == (6,)
    assert np.isnan(result[:2]).all()
    np.testing.assert_allclose(result[2:], [2, 3, 4, 5])


def test_window_edges_are_valid():
    np.testing.assert_allclose(moving_average([2, 4, 9], 3), [5])
    np.testing.assert_allclose(moving_average([2, 4, 9], 1), [2, 4, 9])


def test_out_of_range_windows_are_rejected():
    with pytest.raises(ValueError, match="between 1 and 3"):
        moving_average([2, 4, 9], 0)
    with pytest.raises(ValueError, match="between 1 and 3"):
        moving_average([2, 4, 9], 4)


def test_two_dimensional_input_is_rejected():
    with pytest.raises(ValueError, match="1-D"):
        moving_average([[1, 2], [3, 4]], 2)
