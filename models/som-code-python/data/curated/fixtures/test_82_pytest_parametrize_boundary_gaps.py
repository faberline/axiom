import pytest
from candidate import filter_records, get_boundary_test_cases

def test_boundary_cases_presence():
    cases = get_boundary_test_cases()
    descs = {c["desc"] for c in cases}
    assert "empty_string_category" in descs
    assert "zero_boundary" in descs
    assert "negative_boundary" in descs
    assert any(c["category"] == "" for c in cases)
    assert any(c.get("min_score") == 0.0 for c in cases)
    assert any(c.get("min_score") == -10.0 for c in cases)

def test_filter_records_zero_boundary():
    records = [
        {"id": 1, "score": -10.0, "category": "tech"},
        {"id": 2, "score": 0.0, "category": "tech"},
        {"id": 3, "score": 25.0, "category": "tech"},
    ]
    result = filter_records(records, min_score=0.0)
    scores = [r["score"] for r in result]
    assert scores == [0.0, 25.0]

def test_filter_records_category_exact_match():
    records = [
        {"id": 1, "score": 50.0, "category": "tech"},
        {"id": 2, "score": 50.0, "category": "technology"},
        {"id": 3, "score": 50.0, "category": ""},
    ]
    res_tech = filter_records(records, category="tech")
    assert len(res_tech) == 1
    assert res_tech[0]["id"] == 1
    res_empty = filter_records(records, category="")
    assert len(res_empty) == 1
    assert res_empty[0]["id"] == 3
