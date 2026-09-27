"""Normalize event timestamps to UTC and quarantine unparseable ones."""

from __future__ import annotations

import pandas as pd


def normalize_events(events: pd.DataFrame) -> tuple[pd.DataFrame, pd.DataFrame]:
    """Return (events sorted by UTC timestamp, rows whose timestamp is invalid)."""
    ts = pd.to_datetime(events["ts"], utc=True, errors="coerce", format="ISO8601")
    bad = ts.isna()
    clean = events.loc[~bad]
    clean = clean.sort_values("ts", kind="stable", ignore_index=True)
    return clean, events.loc[bad].reset_index(drop=True)
