import pytest
from fastapi.testclient import TestClient
import candidate
from candidate import app


@pytest.fixture(autouse=True)
def clean_database():
    if hasattr(candidate, "reset_db"):
        candidate.reset_db()
    yield


def test_create_item_success_201():
    client = TestClient(app)
    response = client.post(
        "/items",
        json={"name": "Widget", "price": 19.99, "description": "Useful"},
    )
    # Verifies status code 201 Created (catches miss_1 returning 200)
    assert response.status_code == 201, f"Expected 201, got {response.status_code}"
    data = response.json()
    assert data["name"] == "Widget"
    assert data["price"] == 19.99
    assert data["description"] == "Useful"
    assert "id" in data and isinstance(data["id"], int)

    # Verifies persistence in database (catches miss_3 with uncommitted flush)
    item_id = data["id"]
    get_res = client.get(f"/items/{item_id}")
    assert get_res.status_code == 200, f"Expected 200 on GET, got {get_res.status_code}"
    assert get_res.json()["name"] == "Widget"


def test_create_item_description_persisted():
    client = TestClient(app)
    response = client.post(
        "/items",
        json={"name": "Book", "price": 9.99, "description": "Novel"},
    )
    assert response.status_code == 201
    data = response.json()
    # Catches miss_4 which hardcodes description=None
    assert data["description"] == "Novel", f"Expected 'Novel', got {data['description']}"

    get_res = client.get(f"/items/{data['id']}")
    assert get_res.status_code == 200
    assert get_res.json()["description"] == "Novel"


def test_create_item_rejects_negative_or_zero_price():
    client = TestClient(app)
    # Catches miss_2 which omits gt=0.0 validation
    res_neg = client.post("/items", json={"name": "Widget", "price": -5.0})
    assert res_neg.status_code == 422, f"Expected 422 for negative price, got {res_neg.status_code}"

    res_zero = client.post("/items", json={"name": "Widget", "price": 0.0})
    assert res_zero.status_code == 422, f"Expected 422 for zero price, got {res_zero.status_code}"


def test_create_item_allows_decimal_price_under_one():
    client = TestClient(app)
    # Catches miss_5 which enforces ge=1.0 boundary error
    response = client.post("/items", json={"name": "Cent", "price": 0.50})
    assert response.status_code == 201, f"Expected 201 for price=0.50, got {response.status_code}"
    assert response.json()["price"] == 0.50


def test_create_item_rejects_empty_name():
    client = TestClient(app)
    response = client.post("/items", json={"name": "", "price": 10.0})
    assert response.status_code == 422, f"Expected 422 for empty name, got {response.status_code}"
