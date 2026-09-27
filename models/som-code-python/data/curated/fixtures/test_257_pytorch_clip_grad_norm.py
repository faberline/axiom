import math

import pytest

from candidate import clip_grad_norm


class Param:
    def __init__(self, grad):
        self.grad = grad


def test_large_gradients_are_scaled_jointly():
    a, b = Param([3.0]), Param([4.0])
    total = clip_grad_norm([a, b], 1.0)
    assert total == pytest.approx(5.0)
    assert a.grad[0] == pytest.approx(0.6, rel=1e-5)
    assert b.grad[0] == pytest.approx(0.8, rel=1e-5)


def test_small_gradients_are_left_alone():
    p = Param([0.3, 0.4])
    assert clip_grad_norm([p], 1.0) == pytest.approx(0.5)
    assert p.grad == [0.3, 0.4]


def test_gradients_are_modified_in_place():
    grad = [6.0, 8.0]
    clip_grad_norm([Param(grad)], 5.0)
    assert math.hypot(*grad) == pytest.approx(5.0, rel=1e-5)


def test_params_without_gradients_are_skipped():
    p = Param([3.0, 4.0])
    assert clip_grad_norm(iter([Param(None), p]), 10.0) == pytest.approx(5.0)


def test_non_finite_norm_raises_by_default():
    with pytest.raises(RuntimeError, match="non-finite"):
        clip_grad_norm([Param([math.inf])], 1.0)


def test_non_finite_norm_can_be_reported_instead():
    p = Param([math.nan, 1.0])
    total = clip_grad_norm([p], 1.0, error_if_nonfinite=False)
    assert math.isnan(total)
    assert p.grad[1] == 1.0


def test_max_norm_must_be_positive():
    with pytest.raises(ValueError, match="max_norm"):
        clip_grad_norm([Param([1.0])], 0.0)
