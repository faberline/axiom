"""Aggregate signups into a complete daily UTC series with zero-filled gaps."""

from __future__ import annotations

import pandas as pd


def daily_signups(events: pd.DataFrame, start: str, end: str) -> pd.Series[int]:
    """Count signups per UTC day from ``start`` to ``end`` inclusive."""
    if pd.Timestamp(start) > pd.Timestamp(end):
        raise ValueError("start must not be after end")
    stamps = pd.to_datetime(events["created_at"], utc=True)
    ones = pd.Series(1, index=pd.DatetimeIndex(stamps)).sort_index()
    counts = ones.resample("D").count()
    days = pd.date_range(start, end, freq="D", tz="UTC", name="day")
    return counts.reindex(days).astype("int64").rename("signups")
