"""Summarize sales per region with named aggregations, keeping missing regions."""

from __future__ import annotations

import pandas as pd

UNKNOWN_REGION = "unknown"


def region_summary(sales: pd.DataFrame) -> pd.DataFrame:
    """Return orders, revenue and mean ticket per region, highest revenue first."""
    summary = (
        sales.groupby("region", dropna=False)
        .agg(
            orders=("order_id", "nunique"),
            revenue=("amount", "sum"),
            avg_ticket=("amount", "mean"),
        )
        .reset_index()
    )
    summary["avg_ticket"] = summary["avg_ticket"].round(2)
    return summary.sort_values(
        ["revenue", "region"], ascending=[False, True], ignore_index=True
    )
