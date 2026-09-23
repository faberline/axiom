"""Payment model whose JSON Schema carries examples and PII markers."""

from __future__ import annotations

from typing import Any

from pydantic import BaseModel, Field


def augment_schema(schema: dict[str, Any]) -> None:
    """Add examples and ownership metadata, and mark the account number as PII."""
    schema["examples"] = [
        {
            "transaction_id": "txn_123456",
            "amount": 99.95,
            "account_number": "ACC-987654",
        }
    ]
    schema["x-metadata"] = {
        "classification": "restricted",
        "owner": "billing-core",
    }
    properties = schema.get("properties")
    if isinstance(properties, dict) and "account_number" in properties:
        properties["account_number"]["x-pii"] = True


class PaymentTransaction(BaseModel):
    """A payment whose schema is extended by ``augment_schema``."""

    transaction_id: str
    amount: float = Field(
        ..., gt=0.0, description="Transaction amount in USD, strictly positive"
    )
    account_number: str

    class Config:
        extra = "forbid"
        schema_extra = augment_schema
