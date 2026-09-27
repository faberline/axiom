"""Exchange snake_case order models with a camelCase JSON API."""

from datetime import datetime
from typing import Any

from pydantic import BaseModel, ConfigDict, Field
from pydantic.alias_generators import to_camel


class ApiModel(BaseModel):
    """Base model that speaks camelCase on the wire and snake_case in Python."""

    model_config = ConfigDict(
        alias_generator=to_camel,
        validate_by_alias=True,
        serialize_by_alias=True,
    )


class LineItem(ApiModel):
    """One product line on an order."""

    product_id: str
    unit_price_cents: int = Field(ge=0)
    quantity: int = Field(default=1, ge=1)


class Order(ApiModel):
    """A customer order as the storefront API sends it."""

    order_id: str
    customer_email: str
    line_items: list[LineItem] = Field(min_length=1)
    created_at: datetime
    is_gift: bool = False

    def total_cents(self) -> int:
        """Sum of price times quantity over every line."""
        return sum(item.unit_price_cents * item.quantity for item in self.line_items)


def to_payload(order: Order) -> dict[str, Any]:
    """Render an order as the JSON-ready camelCase dict the API expects."""
    return order.model_dump(mode="json")
