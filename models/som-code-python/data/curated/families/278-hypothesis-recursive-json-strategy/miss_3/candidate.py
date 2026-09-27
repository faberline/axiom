"""Canonical JSON helpers with a recursive Hypothesis strategy for documents."""

from __future__ import annotations

import json
from typing import Any

from hypothesis import strategies as st


def json_values(max_leaves: int = 10) -> st.SearchStrategy[Any]:
    """Return nested lists and dicts over JSON-safe scalars."""
    scalars = (
        st.none()
        | st.booleans()
        | st.integers(min_value=-(2**53), max_value=2**53)
        | st.floats(allow_nan=False, allow_infinity=False)
        | st.text(max_size=8)
    )
    return st.recursive(
        scalars,
        lambda children: (
            st.lists(children, max_size=4)
            | st.dictionaries(st.text(max_size=6), children, max_size=4)
        ),
        max_leaves=max_leaves,
    )


def depth(value: Any) -> int:
    """Return container nesting depth; scalars are 0 and [] is 1."""
    if isinstance(value, dict):
        return 1 + max((depth(item) for item in value.values()), default=0)
    if isinstance(value, list):
        return 1 + max((depth(item) for item in value), default=-1)
    return 0


def canonical_dumps(value: Any) -> str:
    """Dump with sorted keys, no spaces, raw UTF-8, and NaN rejected."""
    return json.dumps(
        value,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=False,
        allow_nan=False,
    )
