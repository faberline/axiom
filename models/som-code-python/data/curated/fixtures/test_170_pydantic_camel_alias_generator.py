from datetime import UTC, datetime

import pytest
from pydantic import ValidationError

from candidate import LineItem, Order, to_payload

WIRE = (
    '{"orderId": "o-1", "customerEmail": "a@example.com", "createdAt":'
    ' "2026-09-24T10:00:00Z", "lineItems": [{"productId": "p-1",'
    ' "unitPriceCents": 250, "quantity": 2}, {"productId": "p-2",'
    ' "unitPriceCents": 100}], "couponCode": "SAVE"}'
)


def test_camel_case_input_is_accepted():
    order = Order.model_validate_json(WIRE)
    assert order.order_id == "o-1"
    assert order.line_items[1].quantity == 1
    assert order.is_gift is False
    assert order.total_cents() == 600


def test_snake_case_names_work_in_python():
    order = Order(
        order_id="o-2",
        customer_email="b@example.com",
        created_at=datetime(2026, 1, 2, tzinfo=UTC),
        line_items=[LineItem(product_id="p", unit_price_cents=5)],
    )
    assert order.total_cents() == 5


def test_payload_uses_camel_case_and_json_types():
    payload = to_payload(Order.model_validate_json(WIRE))
    assert set(payload) == {
        "orderId",
        "customerEmail",
        "lineItems",
        "createdAt",
        "isGift",
    }
    assert payload["lineItems"][0] == {
        "productId": "p-1",
        "unitPriceCents": 250,
        "quantity": 2,
    }
    assert payload["createdAt"] == "2026-09-24T10:00:00Z"


def test_payload_round_trips():
    order = Order.model_validate_json(WIRE)
    assert Order.model_validate(to_payload(order)) == order


def test_quantity_must_be_at_least_one():
    with pytest.raises(ValidationError):
        LineItem.model_validate({"productId": "p", "unitPriceCents": 1, "quantity": 0})


def test_order_needs_a_line_item():
    with pytest.raises(ValidationError):
        Order.model_validate(
            {
                "orderId": "o-3",
                "customerEmail": "c@example.com",
                "createdAt": "2026-09-24T10:00:00Z",
                "lineItems": [],
            }
        )
