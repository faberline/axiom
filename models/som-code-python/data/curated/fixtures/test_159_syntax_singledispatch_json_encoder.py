import json
from datetime import UTC, date, datetime
from decimal import Decimal

import pytest

from candidate import encode


def test_plain_values_pass_through():
    assert encode(None) is None
    assert encode(True) is True
    assert encode(7) == 7
    assert encode("x") == "x"
    assert encode(1.5) == 1.5


def test_decimal_keeps_its_digits():
    assert encode(Decimal("1.10")) == "1.10"


def test_dates_and_aware_datetimes_use_iso_format():
    assert encode(date(2026, 9, 24)) == "2026-09-24"
    stamp = datetime(2026, 9, 24, 8, 30, tzinfo=UTC)
    assert encode(stamp) == "2026-09-24T08:30:00+00:00"


def test_naive_datetime_is_rejected():
    with pytest.raises(ValueError, match="naive datetime"):
        encode(datetime(2026, 9, 24, 8, 30))


@pytest.mark.parametrize("bad", [float("nan"), float("inf")])
def test_non_finite_floats_are_rejected(bad):
    with pytest.raises(ValueError, match="non-finite"):
        encode(bad)


def test_nested_containers_are_encoded_recursively():
    order = {"id": 3, "lines": ({"price": Decimal("2.50")}, [date(2026, 1, 2)])}
    result = encode(order)
    assert result == {"id": 3, "lines": [{"price": "2.50"}, ["2026-01-02"]]}
    assert json.loads(json.dumps(result)) == result


def test_non_string_keys_are_rejected():
    with pytest.raises(TypeError, match="keys must be strings, got int"):
        encode({1: "a"})


def test_unregistered_types_are_rejected():
    with pytest.raises(TypeError, match="cannot encode set"):
        encode({1, 2})
