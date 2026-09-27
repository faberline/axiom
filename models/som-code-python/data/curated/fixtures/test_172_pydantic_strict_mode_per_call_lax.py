import json
from decimal import Decimal

import pytest
from pydantic import ValidationError

from candidate import parse_order_form, parse_order_json


def raw(**overrides):
    fields = {"symbol": "AAPL", "quantity": 10, "limit_price": "12.50"}
    fields.update(overrides)
    return json.dumps(fields)


def test_well_typed_json_is_accepted():
    order = parse_order_json(raw())
    assert order.quantity == 10
    assert order.limit_price == Decimal("12.50")
    assert order.post_only is False


@pytest.mark.parametrize(
    "overrides",
    [{"quantity": "10"}, {"quantity": 10.0}, {"post_only": 1}, {"post_only": "true"}],
)
def test_json_types_are_not_coerced(overrides):
    with pytest.raises(ValidationError):
        parse_order_json(raw(**overrides))


def test_form_strings_are_coerced():
    order = parse_order_form(
        {"symbol": "MSFT", "quantity": "3", "limit_price": "99.9", "post_only": "true"}
    )
    assert order.quantity == 3
    assert order.limit_price == Decimal("99.9")
    assert order.post_only is True


@pytest.mark.parametrize("price", ["12.505", "0", "-1.00"])
def test_price_is_positive_with_two_decimals(price):
    with pytest.raises(ValidationError):
        parse_order_json(raw(limit_price=price))
    with pytest.raises(ValidationError):
        parse_order_form({"symbol": "A", "quantity": "1", "limit_price": price})


@pytest.mark.parametrize("quantity", [0, -5])
def test_quantity_must_be_positive(quantity):
    with pytest.raises(ValidationError):
        parse_order_json(raw(quantity=quantity))


def test_unknown_fields_are_rejected():
    with pytest.raises(ValidationError):
        parse_order_json(raw(side="buy"))
