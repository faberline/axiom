from decimal import Decimal

import pytest

from candidate import Money


class Wildcard:
    def __eq__(self, other):
        return True

    __hash__ = None


def test_equal_values_hash_alike_and_deduplicate():
    a = Money("10", "usd")
    b = Money(Decimal("10.00"), " USD ")
    assert a == b
    assert hash(a) == hash(b)
    assert len({a, b, Money(10, "EUR")}) == 2
    assert {a: "x"}[b] == "x"


def test_rounding_is_bankers_rounding_to_cents():
    assert Money("1.005", "USD").amount == Decimal("1.00")
    assert Money("1.015", "USD").amount == Decimal("1.02")


def test_unknown_types_defer_to_the_other_operand():
    assert Money(1, "USD") == Wildcard()
    assert Money(1, "USD") != "1.00 USD"


def test_addition_keeps_the_currency():
    assert Money("0.10", "EUR") + Money("0.20", "eur") == Money("0.30", "EUR")


def test_mixed_currency_addition_is_rejected():
    with pytest.raises(ValueError, match="currency mismatch: USD \\+ EUR"):
        Money(1, "USD") + Money(1, "EUR")


@pytest.mark.parametrize("code", ["US", "USDX", "12$", ""])
def test_bad_currency_codes(code):
    with pytest.raises(ValueError, match="bad currency code"):
        Money(1, code)


def test_repr_round_trips_the_normalized_value():
    assert repr(Money("2.5", "gbp")) == "Money('2.50', 'GBP')"
