"""Filter scored records and enumerate the boundary cases for the filter."""

from typing import Any


def filter_records(
    records: list[dict[str, Any]],
    min_score: float | None = None,
    category: str | None = None,
) -> list[dict[str, Any]]:
    """Keep records meeting min_score and category; None disables a filter."""
    filtered: list[dict[str, Any]] = []
    for r in records:
        score = r.get("score")
        cat = r.get("category")
        if min_score is not None and (score is None or score < min_score):
            continue
        if category is not None and cat != category:
            continue
        filtered.append(r)
    return filtered


def get_boundary_test_cases() -> list[dict[str, Any]]:
    """Return filter cases around zero, negatives, and empty strings."""
    return [
        {
            "desc": "standard_match",
            "min_score": 50.0,
            "category": "tech",
            "expected_count": 1,
        },
        {
            "desc": "high_boundary",
            "min_score": 80.0,
            "category": None,
            "expected_count": 1,
        },
        {
            "desc": "mid_boundary",
            "min_score": 30.0,
            "category": None,
            "expected_count": 2,
        },
        {
            "desc": "empty_string_category",
            "min_score": None,
            "category": "",
            "expected_count": 1,
        },
        {
            "desc": "all_none_filter",
            "min_score": None,
            "category": None,
            "expected_count": 4,
        },
    ]
