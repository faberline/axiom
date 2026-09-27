import pytest
from fastapi.testclient import TestClient
import candidate
from candidate import app


@pytest.fixture(autouse=True)
def clean_database():
    if hasattr(candidate, "reset_db"):
        candidate.reset_db()
    yield


def test_lookup_existing_active_item_returns_200():
    client = TestClient(app)
    res = client.get("/items/1")
    assert res.status_code == 200, f"Expected 200 for item 1, got {res.status_code}"
    data = res.json()
    assert data["id"] == 1
    assert data["name"] == "Active Widget"
    assert data["is_active"] is True


def test_lookup_nonexistent_item_returns_404():
    client = TestClient(app)
    res = client.get("/items/999")
    # Catches miss_1 (200 with None), miss_2 (400 Bad Request), miss_4 (200 returning item 1)
    assert res.status_code == 404, f"Expected 404 for missing item, got {res.status_code}"
    assert res.json().get("detail") == "Item not found"


def test_lookup_path_param_zero_rejected_with_422():
    client = TestClient(app)
    # Path parameter must enforce ge=1; item_id=0 must return 422 Unprocessable Entity (catches miss_3)
    res = client.get("/items/0")
    assert res.status_code == 422, f"Expected 422 for item_id=0, got {res.status_code}"


def test_lookup_inactive_item_returns_404():
    client = TestClient(app)
    # Inactive/soft-deleted items must return 404 Not Found (catches miss_5 returning 200)
    res = client.get("/items/2")
    assert res.status_code == 404, f"Expected 404 for inactive item 2, got {res.status_code}"
    assert res.json().get("detail") == "Item not found"
