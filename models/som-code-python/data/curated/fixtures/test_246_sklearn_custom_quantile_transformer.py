import numpy as np
import pytest
from sklearn.base import clone
from sklearn.exceptions import NotFittedError
from sklearn.linear_model import LinearRegression
from sklearn.pipeline import make_pipeline

from candidate import QuantileClipper

X = np.column_stack([np.arange(101.0), np.arange(101.0) * 10])


def test_defaults_and_clone():
    clipper = QuantileClipper()
    assert clipper.get_params() == {"lower": 0.01, "upper": 0.99}
    copy = clone(QuantileClipper(lower=0.1, upper=0.8))
    assert copy.get_params() == {"lower": 0.1, "upper": 0.8}


def test_bounds_are_learned_per_column():
    clipper = QuantileClipper(lower=0.1, upper=0.9).fit(X)
    np.testing.assert_allclose(clipper.lower_, [10.0, 100.0])
    np.testing.assert_allclose(clipper.upper_, [90.0, 900.0])
    out = clipper.transform([[0.0, 5000.0], [50.0, 500.0]])
    np.testing.assert_allclose(out, [[10.0, 900.0], [50.0, 500.0]])


def test_transform_returns_a_copy():
    data = X.copy()
    QuantileClipper(lower=0.2, upper=0.8).fit_transform(data)
    np.testing.assert_array_equal(data, X)


def test_invalid_quantiles_fail_at_fit_not_init():
    clipper = QuantileClipper(lower=0.9, upper=0.1)
    with pytest.raises(ValueError, match="lower < upper"):
        clipper.fit(X)


def test_transform_requires_fit_and_matching_width():
    with pytest.raises(NotFittedError):
        QuantileClipper().transform(X)
    clipper = QuantileClipper().fit(X)
    with pytest.raises(ValueError, match="expected 2 features, got 3"):
        clipper.transform(np.ones((2, 3)))


def test_works_inside_a_pipeline():
    y = X[:, 0] * 2
    model = make_pipeline(QuantileClipper(), LinearRegression()).fit(X, y)
    assert model.predict(X[:3]).shape == (3,)
