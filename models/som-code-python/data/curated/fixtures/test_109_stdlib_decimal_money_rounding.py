from decimal import Decimal

import pytest

from candidate import split_evenly, to_money


def test_strings_ints_and_decimals_round_half_up_to_cents():
    assert to_money("0.125") == Decimal("0.13")
    assert str(to_money("2.675")) == "2.68"
    assert str(to_money(3)) == "3.00"
    assert to_money(Decimal("1.004")) == Decimal("1.00")


def test_floats_and_bools_are_rejected():
    with pytest.raises(TypeError, match="unsupported money value: float"):
        to_money(0.1)
    with pytest.raises(TypeError, match="unsupported money value: bool"):
        to_money(True)


def test_garbage_and_non_finite_values_raise_value_error():
    with pytest.raises(ValueError, match="not a number"):
        to_money("12,50")
    with pytest.raises(ValueError, match="not a finite amount"):
        to_money("NaN")


def test_split_gives_leftover_cents_to_the_first_parts():
    assert split_evenly(Decimal("1.00"), 3) == [Decimal("0.34"), Decimal("0.33"), Decimal("0.33")]
    assert sum(split_evenly(Decimal("10.01"), 4)) == Decimal("10.01")


def test_split_rejects_zero_parts():
    with pytest.raises(ValueError, match="parts must be at least 1"):
        split_evenly(Decimal("5"), 0)
