import numpy as np
import pytest

from candidate import winsorize

SAMPLE = np.arange(101, dtype=np.float64)


def test_default_bounds_are_5th_and_95th_percentiles():
    out = winsorize(SAMPLE)
    assert out.min() == 5.0
    assert out.max() == 95.0
    np.testing.assert_array_equal(out[5:96], SAMPLE[5:96])


def test_custom_bounds():
    out = winsorize([1.0, 2.0, 3.0, 4.0, 100.0], lower=0, upper=75)
    assert out.tolist() == [1.0, 2.0, 3.0, 4.0, 4.0]


def test_nan_is_ignored_and_preserved():
    data = np.append(SAMPLE, np.nan)
    out = winsorize(data, lower=10, upper=90)
    assert np.isnan(out[-1])
    assert np.nanmin(out) == 10.0
    assert np.nanmax(out) == 90.0


def test_input_is_not_mutated():
    data = SAMPLE.copy()
    out = winsorize(data)
    np.testing.assert_array_equal(data, SAMPLE)
    assert out is not data


@pytest.mark.parametrize(("lower", "upper"), [(50, 50), (60, 40), (-1, 50), (5, 101)])
def test_invalid_percentiles_are_rejected(lower, upper):
    with pytest.raises(ValueError, match="percentiles"):
        winsorize(SAMPLE, lower=lower, upper=upper)


def test_bad_shapes_and_empty_input_are_rejected():
    with pytest.raises(ValueError, match="1-D"):
        winsorize(np.ones((2, 2)))
    with pytest.raises(ValueError, match="no finite"):
        winsorize([])
    with pytest.raises(ValueError, match="no finite"):
        winsorize([np.nan, np.nan])
