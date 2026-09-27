import pytest

from candidate import warmup_cosine


def test_warmup_is_linear_and_never_zero():
    f = warmup_cosine(4, 14)
    assert [f(s) for s in range(4)] == [0.25, 0.5, 0.75, 1.0]


def test_peak_right_after_warmup():
    f = warmup_cosine(4, 14)
    assert f(4) == pytest.approx(1.0)


def test_cosine_midpoint_and_end():
    f = warmup_cosine(0, 10)
    assert f(0) == pytest.approx(1.0)
    assert f(5) == pytest.approx(0.5)
    assert f(10) == pytest.approx(0.0)


def test_min_ratio_scales_the_decay():
    f = warmup_cosine(0, 10, min_ratio=0.2)
    assert f(5) == pytest.approx(0.6)
    assert f(10) == pytest.approx(0.2)


def test_factor_stays_at_the_floor_after_total_steps():
    f = warmup_cosine(2, 10, min_ratio=0.1)
    assert f(15) == pytest.approx(0.1)
    assert f(1000) == pytest.approx(0.1)


def test_arguments_are_validated():
    with pytest.raises(ValueError, match="warmup_steps"):
        warmup_cosine(-1, 10)
    with pytest.raises(ValueError, match="total_steps"):
        warmup_cosine(10, 10)
    with pytest.raises(ValueError, match="min_ratio"):
        warmup_cosine(0, 10, min_ratio=1.5)
    warmup_cosine(0, 1, min_ratio=1.0)
