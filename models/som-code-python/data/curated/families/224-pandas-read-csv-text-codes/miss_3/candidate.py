"""Load a customer CSV without mangling codes or reading NA as missing."""

from __future__ import annotations

import io

import pandas as pd

REQUIRED = ("customer_id", "zip", "country", "balance")
TEXT_COLUMNS = {"customer_id": str, "zip": str, "country": str}
MISSING_MARKERS = [""]


def load_customers(text: str) -> pd.DataFrame:
    """Parse ``text`` keeping code columns as strings; only blanks are missing."""
    frame = pd.read_csv(
        io.StringIO(text),
        dtype=TEXT_COLUMNS,
        keep_default_na=False,
    )
    missing = sorted(set(REQUIRED) - set(frame.columns))
    if missing:
        raise ValueError(f"missing columns: {', '.join(missing)}")
    return frame
