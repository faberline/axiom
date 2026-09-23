import pytest
from fastapi.testclient import TestClient
import candidate
from candidate import app


@pytest.fixture(autouse=True)
def clean_database():
    """Ensure database is fresh for every test in StaticPool in-memory SQLite."""
    if hasattr(candidate, "reset_db"):
        candidate.reset_db()
    elif hasattr(candidate, "Base") and hasattr(candidate, "engine"):
        candidate.Base.metadata.drop_all(bind=candidate.engine)
        candidate.Base.metadata.create_all(bind=candidate.engine)
    yield


@pytest.fixture
def client():
    """Default client with per-request session lifecycle (used for persistence tests)."""
    return TestClient(app)


@pytest.fixture
def shared_session_client():
    """Client overriding get_db with a shared session across consecutive operations.
    
    Verifies that route handlers cleanly roll back uncommitted changes on failure.
    If db.rollback() is omitted, dirty session state leaks into subsequent operations.
    """
    shared_session = candidate.session_factory()
    candidate.app.dependency_overrides[candidate.get_db] = lambda: shared_session
    with TestClient(app) as test_client:
        yield test_client
    candidate.app.dependency_overrides.clear()
    shared_session.close()


def test_atomic_rollback_on_insufficient_stock(shared_session_client):
    p1 = shared_session_client.post("/products", json={"name": "Prod A", "stock": 10, "price": 20.0}).json()
    p2 = shared_session_client.post("/products", json={"name": "Prod B", "stock": 2, "price": 50.0}).json()

    order_payload = {
        "items": [
            {"product_id": p1["id"], "quantity": 5},
            {"product_id": p2["id"], "quantity": 5},
        ]
    }
    order_res = shared_session_client.post("/orders", json=order_payload)
    assert order_res.status_code == 400

    # If db.rollback() is called (gold), p1.stock is rolled back to 10.
    # If db.rollback() is omitted (miss_2), p1.stock remains 5 in the shared session!
    p1_check = shared_session_client.get(f"/products/{p1['id']}").json()
    assert p1_check["stock"] == 10

    p2_check = shared_session_client.get(f"/products/{p2['id']}").json()
    assert p2_check["stock"] == 2

    orders = shared_session_client.get("/orders").json()
    assert not any(any(item["product_id"] == p1["id"] for item in o["items"]) for o in orders)


def test_subsequent_order_succeeds_after_rollback(shared_session_client):
    p = shared_session_client.post("/products", json={"name": "Prod C", "stock": 5, "price": 10.0}).json()

    fail_res = shared_session_client.post(
        "/orders",
        json={"items": [{"product_id": p["id"], "quantity": 10}]},
    )
    assert fail_res.status_code == 400

    ok_res = shared_session_client.post(
        "/orders",
        json={"items": [{"product_id": p["id"], "quantity": 2}]},
    )
    assert ok_res.status_code == 201

    p_check = shared_session_client.get(f"/products/{p['id']}").json()
    assert p_check["stock"] == 3

    # In a clean rollback, only the valid order is persisted.
    # If rollback was omitted (miss_2), the uncommitted order from fail_res leaks
    # into the session and gets committed on ok_res, producing 2 orders!
    orders = shared_session_client.get("/orders").json()
    assert len(orders) == 1


def test_order_exact_stock_boundary_succeeds(client):
    p = client.post("/products", json={"name": "Prod D", "stock": 5, "price": 15.0}).json()

    order_res = client.post(
        "/orders",
        json={"items": [{"product_id": p["id"], "quantity": 5}]},
    )
    assert order_res.status_code == 201

    p_check = client.get(f"/products/{p['id']}").json()
    assert p_check["stock"] == 0


def test_insufficient_stock_fails_without_negative_stock(client):
    p = client.post("/products", json={"name": "Prod E", "stock": 3, "price": 25.0}).json()

    order_res = client.post(
        "/orders",
        json={"items": [{"product_id": p["id"], "quantity": 10}]},
    )
    assert order_res.status_code == 400

    p_check = client.get(f"/products/{p['id']}").json()
    assert p_check["stock"] == 3


def test_valid_order_persisted_to_database(client):
    """Uses client (separate sessions) to ensure db.commit() was called rather than just db.flush() (catches miss_5)."""
    p = client.post("/products", json={"name": "Prod F", "stock": 20, "price": 30.0}).json()

    order_res = client.post(
        "/orders",
        json={"items": [{"product_id": p["id"], "quantity": 3}]},
    )
    assert order_res.status_code == 201
    order_id = order_res.json()["id"]

    get_order_res = client.get(f"/orders/{order_id}")
    assert get_order_res.status_code == 200
    assert get_order_res.json()["id"] == order_id

    p_check = client.get(f"/products/{p['id']}").json()
    assert p_check["stock"] == 17
