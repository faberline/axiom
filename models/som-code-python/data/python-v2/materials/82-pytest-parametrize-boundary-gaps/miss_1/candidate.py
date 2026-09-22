from typing import Any, Dict, List, Optional

def filter_records(
    records: List[Dict[str, Any]],
    min_score: Optional[float] = None,
    category: Optional[str] = None,
) -> List[Dict[str, Any]]:
    filtered: List[Dict[str, Any]] = []
    for r in records:
        score = r.get("score")
        cat = r.get("category")
        if min_score is not None:
            if score is None or score < min_score:
                continue
        if category is not None:
            if cat != category:
                continue
        filtered.append(r)
    return filtered

def get_boundary_test_cases() -> List[Dict[str, Any]]:
    return [
        {"desc": "standard_match", "min_score": 50.0, "category": "tech", "expected_count": 1},
        {"desc": "zero_boundary", "min_score": 0.0, "category": None, "expected_count": 3},
        {"desc": "negative_boundary", "min_score": -10.0, "category": None, "expected_count": 4},
        {"desc": "all_none_filter", "min_score": None, "category": None, "expected_count": 4},
    ]
