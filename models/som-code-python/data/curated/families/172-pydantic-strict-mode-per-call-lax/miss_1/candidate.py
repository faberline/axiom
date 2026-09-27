"""Validate limit orders strictly from JSON and leniently from HTML forms."""

from collections.abc import Mapping
from decimal import Decimal

from pydantic import BaseModel, ConfigDict, Field


class LimitOrder(BaseModel):
    """A limit order; wrong JSON types are rejected instead of coerced."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    symbol: str = Field(min_length=1, max_length=8)
    quantity: int = Field(gt=0)
    limit_price: Decimal = Field(gt=0, decimal_places=2)
    post_only: bool = False


def parse_order_json(raw: str) -> LimitOrder:
    """Parse an API body; "10" for quantity or 1 for post_only is an error."""
    return LimitOrder.model_validate_json(raw)


def parse_order_form(fields: Mapping[str, str]) -> LimitOrder:
    """Parse form fields, where every value arrives as a string."""
    return LimitOrder.model_validate(dict(fields), strict=False)
