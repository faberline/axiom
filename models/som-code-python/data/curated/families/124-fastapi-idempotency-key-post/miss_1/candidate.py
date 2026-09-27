"""Payments API that makes POST retries safe with an Idempotency-Key header."""

import hashlib
import json
from typing import Annotated

from fastapi import FastAPI, Header, Response, status
from pydantic import BaseModel, Field


class PaymentIn(BaseModel):
    """Requested charge."""

    account: str = Field(min_length=1)
    amount_cents: int = Field(gt=0)


class PaymentOut(BaseModel):
    """Recorded charge."""

    id: int
    account: str
    amount_cents: int


PAYMENTS: list[PaymentOut] = []
SEEN: dict[str, tuple[str, PaymentOut]] = {}
app = FastAPI(title="Payments")


def reset_db() -> None:
    """Forget every payment and idempotency key."""
    PAYMENTS.clear()
    SEEN.clear()


def fingerprint(body: PaymentIn) -> str:
    """Return a stable digest of the request payload."""
    return hashlib.sha256(
        json.dumps(body.model_dump(), sort_keys=True).encode()
    ).hexdigest()


@app.post("/payments", response_model=PaymentOut, status_code=status.HTTP_201_CREATED)
def create_payment(
    body: PaymentIn,
    response: Response,
    idempotency_key: Annotated[str, Header(min_length=8, max_length=64)],
) -> PaymentOut:
    """Charge once per key; a retry with the same payload replays the result."""
    digest = fingerprint(body)
    if (previous := SEEN.get(idempotency_key)) is not None:
        _, payment = previous
        response.headers["Idempotent-Replayed"] = "true"
        return payment
    payment = PaymentOut(id=len(PAYMENTS) + 1, **body.model_dump())
    PAYMENTS.append(payment)
    SEEN[idempotency_key] = (digest, payment)
    return payment
