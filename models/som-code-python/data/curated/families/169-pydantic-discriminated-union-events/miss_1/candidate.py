"""Validate webhook events whose shape is chosen by their kind tag."""

from collections.abc import Iterable
from typing import Literal

from pydantic import BaseModel, ConfigDict, Field, TypeAdapter


class _Event(BaseModel):
    model_config = ConfigDict(extra="forbid", frozen=True)

    id: str = Field(min_length=1)


class PaymentSucceeded(_Event):
    """A captured payment."""

    kind: Literal["payment.succeeded"]
    amount_cents: int = Field(gt=0)
    currency: Literal["usd", "eur"]


class PaymentRefunded(_Event):
    """Money returned to the customer."""

    kind: Literal["payment.refunded"]
    amount_cents: int = Field(gt=0)
    reason: str = "requested_by_customer"


class CustomerDeleted(_Event):
    """A customer removed from the account."""

    kind: Literal["customer.deleted"]
    customer_id: str = Field(min_length=1)


AnyEvent = PaymentSucceeded | PaymentRefunded | CustomerDeleted
_EVENTS: TypeAdapter[AnyEvent] = TypeAdapter(AnyEvent)


def parse_event(payload: bytes) -> AnyEvent:
    """Parse one JSON webhook body into its event model."""
    return _EVENTS.validate_json(payload)


def net_revenue(payloads: Iterable[bytes]) -> int:
    """Captured minus refunded cents across the payloads."""
    total = 0
    for payload in payloads:
        event = parse_event(payload)
        if isinstance(event, PaymentSucceeded):
            total += event.amount_cents
        elif isinstance(event, PaymentRefunded):
            total -= event.amount_cents
    return total
