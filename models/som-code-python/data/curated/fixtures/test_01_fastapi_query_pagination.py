import pytest
from fastapi.testclient import TestClient
import candidate
from candidate import app


@pytest.fixture(autouse=True)
def clean_database():
    if hasattr(candidate, "reset_db"):
        candidate.reset_db()
    yield


def test_pagination_first_page_correct_offset():
    client = TestClient(app)
    res = client.get("/items?page=1&size=5")
    assert res.status_code == 200, f"Expected 200, got {res.status_code}"
    data = res.json()
    assert "items" in data and len(data["items"]) == 5
    # Checks that page 1 starts at ID 1, not ID 6 (catches miss_1 offset off-by-one)
    ids = [item["id"] for item in data["items"]]
    assert ids == [1, 2, 3, 4, 5], f"Expected IDs [1, 2, 3, 4, 5], got {ids}"


def test_pagination_second_page_correct_offset():
    client = TestClient(app)
    # Querying page 2 with size 10 must apply offset 10 and return items with IDs 11..20
    # Closes Challenger 1 mutant gap where candidate hardcodes offset=0 or ignores page
    res = client.get("/items?page=2&size=10")
    assert res.status_code == 200, f"Expected 200, got {res.status_code}"
    data = res.json()
    assert "items" in data and len(data["items"]) == 10
    ids = [item["id"] for item in data["items"]]
    expected_ids = [11, 12, 13, 14, 15, 16, 17, 18, 19, 20]
    assert ids == expected_ids, f"Expected IDs {expected_ids}, got {ids}"
    assert data["page"] == 2
    assert data["size"] == 10
    assert data["total"] == 25
    assert data["pages"] == 3


def test_pagination_total_count_not_truncated():
    client = TestClient(app)
    res = client.get("/items?page=1&size=10")
    assert res.status_code == 200
    data = res.json()
    # Total must reflect all 25 rows in DB, not just page length (catches miss_2)
    assert data["total"] == 25, f"Expected total 25, got {data['total']}"
    assert len(data["items"]) == 10


def test_pagination_pages_calculation_ceiling():
    client = TestClient(app)
    res = client.get("/items?page=1&size=10")
    assert res.status_code == 200
    data = res.json()
    # 25 total items with size 10 must yield 3 pages (ceil(25/10) = 3) (catches miss_3)
    assert data["pages"] == 3, f"Expected 3 pages, got {data['pages']}"


def test_pagination_deterministic_ascending_order():
    client = TestClient(app)
    res = client.get("/items?page=1&size=5")
    assert res.status_code == 200
    data = res.json()
    ids = [item["id"] for item in data["items"]]
    # Catches miss_4 which sorts descending
    assert ids == [1, 2, 3, 4, 5], f"Expected ascending order [1, 2, 3, 4, 5], got {ids}"
    assert all(ids[i] < ids[i + 1] for i in range(len(ids) - 1))


def test_pagination_rejects_page_zero():
    client = TestClient(app)
    # Page must be >= 1; page=0 must fail validation with HTTP 422 (catches miss_5)
    res = client.get("/items?page=0&size=10")
    assert res.status_code == 422, f"Expected 422 for page=0, got {res.status_code}"
