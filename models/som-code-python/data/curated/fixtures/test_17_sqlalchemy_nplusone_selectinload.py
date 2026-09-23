import candidate
from fastapi.testclient import TestClient
import pytest


@pytest.fixture(autouse=True)
def clean_database():
    if hasattr(candidate, "reset_db"):
        candidate.reset_db()
    elif hasattr(candidate, "Base") and hasattr(candidate, "engine"):
        candidate.Base.metadata.drop_all(bind=candidate.engine)
        candidate.Base.metadata.create_all(bind=candidate.engine)
    yield


@pytest.fixture
def client():
    return TestClient(candidate.app)


def test_orders_query_count_strictly_two(client):
    candidate.QueryCounter.reset()
    res = client.get("/orders")
    assert res.status_code == 200
    assert candidate.QueryCounter.count == 2


def test_orders_returns_all_five_orders(client):
    res = client.get("/orders")
    assert res.status_code == 200
    orders = res.json()
    assert len(orders) == 5
    assert orders[0]["order_number"] == "ORD-001"
    assert orders[-1]["order_number"] == "ORD-005"


def test_orders_all_line_items_serialized(client):
    res = client.get("/orders")
    assert res.status_code == 200
    orders = res.json()
    for order in orders:
        assert len(order["items"]) == 2
        assert order["items"][0]["quantity"] == 1
        assert order["items"][1]["quantity"] == 2


def test_orders_content_verification(client):
    res = client.get("/orders")
    assert res.status_code == 200
    orders = res.json()
    first_order = orders[0]
    assert first_order["status"] == "completed"
    item_names = [item["product_name"] for item in first_order["items"]]
    assert "Item 1-A" in item_names
    assert "Item 1-B" in item_names
