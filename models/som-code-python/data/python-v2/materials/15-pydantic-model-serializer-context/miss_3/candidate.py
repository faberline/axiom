from __future__ import annotations

from typing import Any, Optional
from pydantic import BaseModel, ConfigDict, SerializationInfo, model_serializer


class CustomerRecord(BaseModel):
    model_config = ConfigDict(extra="forbid")

    customer_id: str
    name: str
    credit_card: str
    ssn: str
    notes: Optional[str] = None

    @model_serializer(mode="wrap")
    def serialize_model(self, handler: Any, info: SerializationInfo) -> dict[str, Any]:
        data = handler(self)
        context = info.context if info.context is not None else {}
        role = context.get("role", "public")

        if role == "admin" or "auditor":
            return data

        raw_cc = data.get("credit_card", "")
        last_4 = raw_cc[-4:] if len(raw_cc) >= 4 else raw_cc
        data["credit_card"] = f"****-****-****-{last_4}"

        if role == "auditor":
            data["ssn"] = "***-**-****"
            return data
        else:
            data.pop("ssn", None)
            data.pop("notes", None)
            return data
