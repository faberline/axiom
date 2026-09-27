"""Build a product-by-month revenue matrix with pivot_table."""

from __future__ import annotations

import pandas as pd

TOTAL_LABEL = "total"


def revenue_matrix(sales: pd.DataFrame) -> pd.DataFrame:
    """Rows are products, columns are YYYY-MM months, cells are summed revenue."""
    frame = sales.assign(month=pd.to_datetime(sales["date"]).dt.strftime("%Y-%m"))
    return frame.pivot_table(
        index="product",
        columns="month",
        values="amount",
        aggfunc="max",
        fill_value=0,
        margins=True,
        margins_name=TOTAL_LABEL,
    )
