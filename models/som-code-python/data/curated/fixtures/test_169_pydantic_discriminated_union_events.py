import json

import pytest
from pydantic import ValidationError

from candidate import (
    CustomerDeleted,
    PaymentRefunded,
    PaymentSucceeded,
    net_revenue,
    parse_event,
)


def body(**fields):
    return json.dumps(fields).encode()


def test_payment_succeeded_is_parsed():
    event = parse_event(
        body(id="ev_1", kind="payment.succeeded", amount_cents=1000, currency="usd")
    )
    assert isinstance(event, PaymentSucceeded)
    assert event.amount_cents == 1000


def test_refund_gets_default_reason():
    event = parse_event(body(id="ev_2", kind="payment.refunded", amount_cents=250))
    assert isinstance(event, PaymentRefunded)
    assert event.reason == "requested_by_customer"


def test_unknown_kind_is_a_tag_error():
    with pytest.raises(ValidationError) as info:
        parse_event(body(id="ev_3", kind="invoice.paid"))
    assert [e["type"] for e in info.value.errors()] == ["union_tag_invalid"]


def test_missing_kind_is_a_tag_error():
    with pytest.raises(ValidationError) as info:
        parse_event(body(id="ev_4", customer_id="cus_1"))
    assert [e["type"] for e in info.value.errors()] == ["union_tag_not_found"]


def test_errors_are_reported_for_the_tagged_model_only():
    with pytest.raises(ValidationError) as info:
        parse_event(
            body(id="ev_5", kind="payment.refunded", amount_cents=5, customer_id="c")
        )
    errors = info.value.errors()
    assert len(errors) == 1
    assert errors[0]["type"] == "extra_forbidden"
    assert errors[0]["loc"] == ("payment.refunded", "customer_id")


@pytest.mark.parametrize("amount", [0, -1])
def test_payment_amount_must_be_positive(amount):
    with pytest.raises(ValidationError):
        parse_event(
            body(
                id="ev_6", kind="payment.succeeded", amount_cents=amount, currency="eur"
            )
        )


def test_events_are_frozen():
    event = parse_event(body(id="ev_7", kind="customer.deleted", customer_id="c"))
    assert isinstance(event, CustomerDeleted)
    with pytest.raises(ValidationError):
        event.customer_id = "other"


def test_net_revenue_subtracts_refunds():
    payloads = [
        body(id="a", kind="payment.succeeded", amount_cents=1000, currency="usd"),
        body(id="b", kind="payment.refunded", amount_cents=250),
        body(id="c", kind="customer.deleted", customer_id="cus_9"),
    ]
    assert net_revenue(payloads) == 750
