import pytest
from fastapi.testclient import TestClient
import candidate
from candidate import app


@pytest.fixture(autouse=True)
def reset_candidate_db():
    if hasattr(candidate, "reset_db"):
        candidate.reset_db()
    yield
    if hasattr(candidate, "reset_db"):
        candidate.reset_db()


def test_submit_order_returns_202_accepted():
    client = TestClient(app)
    response = client.post(
        "/orders",
        json={
            "order_id": "ord-202",
            "customer_email": "buyer@example.com",
            "item": "Keyboard",
            "quantity": 2,
        },
    )
    # Verifies status 202 Accepted (catches miss_1 returning 200 OK)
    assert response.status_code == 202, f"Expected 202 Accepted, got {response.status_code}"
    data = response.json()
    assert data["status"] == "accepted"
    assert data["order_id"] == "ord-202"
    assert data["order_status"] == "pending"


def test_background_task_dispatched_and_callable():
    client = TestClient(app)
    # Calling endpoint dispatches background task; TestClient executes it synchronously before return
    # Catches miss_2 which passes None instead of a callable to add_task
    response = client.post(
        "/orders",
        json={
            "order_id": "ord-callable",
            "customer_email": "test@domain.com",
            "item": "Monitor",
            "quantity": 1,
        },
    )
    assert response.status_code == 202


def test_order_status_transitions_from_pending_to_processed():
    client = TestClient(app)
    order_id = "ord-transition"
    post_res = client.post(
        "/orders",
        json={
            "order_id": order_id,
            "customer_email": "alice@company.com",
            "item": "Laptop",
            "quantity": 1,
        },
    )
    assert post_res.status_code == 202

    # After background task execution, the stored order status must be "processed"
    # Catches miss_3 where background worker fails to transition status
    get_res = client.get(f"/orders/{order_id}")
    assert get_res.status_code == 200
    order_data = get_res.json()
    assert order_data["status"] == "processed", (
        f"Expected order status 'processed', got '{order_data['status']}'"
    )


def test_audit_log_records_non_partner_customer():
    client = TestClient(app)
    email = "independent.developer@standard-mail.net"
    response = client.post(
        "/orders",
        json={
            "order_id": "ord-audit",
            "customer_email": email,
            "item": "Desk",
            "quantity": 1,
        },
    )
    assert response.status_code == 202

    # Verify audit log includes event for non-partner customer
    # Catches miss_4 restricting audit logs exclusively to @partner.com
    logs_res = client.get("/audit-logs")
    assert logs_res.status_code == 200
    logs = logs_res.json().get("logs", [])
    matched = [entry for entry in logs if entry.get("customer_email") == email]
    assert len(matched) >= 1, f"Expected audit log entry for {email}, but found none in {logs}"
    assert matched[0]["event"] == "order_processed"


def test_single_item_order_quantity_one_accepted():
    client = TestClient(app)
    # A single-item order with quantity == 1 must be accepted (gt=0)
    # Catches miss_5 which enforces gt=1 and rejects quantity 1 with 422
    response = client.post(
        "/orders",
        json={
            "order_id": "ord-single",
            "customer_email": "single@user.org",
            "item": "Mouse",
            "quantity": 1,
        },
    )
    assert response.status_code == 202, f"Expected 202 for quantity=1, got {response.status_code}"


def test_customer_email_normalized_to_lowercase():
    client = TestClient(app)
    order_id = "ord-casing"
    response = client.post(
        "/orders",
        json={
            "order_id": order_id,
            "customer_email": "John.DOE@Sub.Domain.COM",
            "item": "Headphones",
            "quantity": 3,
        },
    )
    assert response.status_code == 202
    get_res = client.get(f"/orders/{order_id}")
    assert get_res.status_code == 200
    assert get_res.json()["customer_email"] == "john.doe@sub.domain.com"


def test_duplicate_order_id_returns_409_conflict():
    client = TestClient(app)
    order_id = "ord-dup"
    r1 = client.post(
        "/orders",
        json={"order_id": order_id, "customer_email": "u1@test.com", "item": "A", "quantity": 1},
    )
    assert r1.status_code == 202

    r2 = client.post(
        "/orders",
        json={"order_id": order_id, "customer_email": "u2@test.com", "item": "B", "quantity": 2},
    )
    assert r2.status_code == 409
