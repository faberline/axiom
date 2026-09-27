"""Customer record whose serialized fields are masked by the caller's role."""

from __future__ import annotations

from typing import Any

from pydantic import (
    BaseModel,
    ConfigDict,
    SerializerFunctionWrapHandler,
    model_serializer,
)


class CustomerRecord(BaseModel):
    """A customer with payment and identity fields that need masking."""

    model_config = ConfigDict(extra="forbid")

    customer_id: str
    name: str
    credit_card: str
    ssn: str
    notes: str | None = None

    @model_serializer(mode="wrap")
    def serialize_model(self, handler: SerializerFunctionWrapHandler) -> dict[str, Any]:
        """Mask the card for every role but admin, and hide more from the public."""
        data: dict[str, Any] = handler(self)
        context = info.context if info.context is not None else {}
        role = context.get("role", "public")

        if role == "admin":
            return data

        raw_cc = data.get("credit_card", "")
        last_4 = raw_cc[-4:] if len(raw_cc) >= 4 else raw_cc
        data["credit_card"] = f"****-****-****-{last_4}"

        if role == "auditor":
            data["ssn"] = "***-**-****"
            return data
        data.pop("ssn", None)
        data.pop("notes", None)
        return data
