"""Join orders to customers with a validated, audited pandas merge."""

from __future__ import annotations

import pandas as pd


class UnmatchedOrdersError(ValueError):
    """Raised when some orders reference a customer that does not exist."""

    def __init__(self, order_ids: list[int]) -> None:
        super().__init__(f"{len(order_ids)} orders have no customer: {order_ids}")
        self.order_ids = order_ids


def attach_customers(orders: pd.DataFrame, customers: pd.DataFrame) -> pd.DataFrame:
    """Add customer columns to every order, keeping the order row order."""
    merged = orders.merge(
        customers,
        on="customer_id",
        how="left",
        validate="many_to_one",
        indicator=True,
    )
    missing = merged.loc[merged["_merge"] == "left_only", "order_id"]
    if not missing.empty:
        raise UnmatchedOrdersError(sorted(int(i) for i in missing))
    return merged
