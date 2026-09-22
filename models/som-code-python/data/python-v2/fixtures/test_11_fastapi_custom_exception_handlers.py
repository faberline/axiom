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


def test_transfer_success_exact_balance_zero():
    client = TestClient(app)
    # acc_1 has 100.0. Transferring 100.0 leaves exactly 0.0 balance.
    response = client.post(
        "/transfers",
        json={"from_account": "acc_1", "to_account": "acc_2", "amount": 100.0},
    )
    # Verifies status 200 and balance 0.0 (catches miss_5 using <= 0.0 which rejects 100.0)
    assert response.status_code == 200, f"Expected 200 for exact balance transfer, got {response.status_code}"
    data = response.json()
    assert data["status"] == "completed"
    assert data["remaining_balance"] == 0.0
    assert data["amount"] == 100.0

    # Verify recipient received funds
    acc2_res = client.get("/accounts/acc_2")
    assert acc2_res.status_code == 200
    assert acc2_res.json()["balance"] == 150.0


def test_insufficient_funds_status_code_402():
    client = TestClient(app)
    # Attempting to transfer more than balance (100.0 available, requesting 150.0)
    response = client.post(
        "/transfers",
        json={"from_account": "acc_1", "to_account": "acc_2", "amount": 150.0},
    )
    # Verifies status code 402 Payment Required (catches miss_1 returning 200)
    assert response.status_code == 402, f"Expected 402 Payment Required, got {response.status_code}"


def test_insufficient_funds_rfc_envelope_contains_details():
    client = TestClient(app)
    response = client.post(
        "/transfers",
        json={"from_account": "acc_1", "to_account": "acc_2", "amount": 180.0},
    )
    assert response.status_code == 402
    data = response.json()
    assert "error" in data, "Response body must contain 'error' key"
    err = data["error"]
    assert err.get("code") == "INSUFFICIENT_FUNDS"
    assert "message" in err

    # Verifies error envelope includes 'details' dict (catches miss_2 dropping details)
    assert "details" in err and isinstance(err["details"], dict), "Error envelope must include 'details' dict"
    assert err["details"].get("available") == 100.0
    assert err["details"].get("requested") == 180.0


def test_insufficient_funds_handler_returns_json_response():
    client = TestClient(app)
    # When exception handler returns a raw dict instead of JSONResponse, Starlette fails (catches miss_3)
    response = client.post(
        "/transfers",
        json={"from_account": "acc_1", "to_account": "acc_2", "amount": 999.0},
    )
    assert response.status_code == 402
    assert "application/json" in response.headers.get("content-type", "")
    assert isinstance(response.json(), dict)


def test_request_id_preserved_from_header_in_error_envelope():
    client = TestClient(app)
    custom_trace_id = "trace-uuid-11-custom-9999"
    response = client.post(
        "/transfers",
        json={"from_account": "acc_1", "to_account": "acc_2", "amount": 500.0},
        headers={"X-Request-ID": custom_trace_id},
    )
    assert response.status_code == 402
    data = response.json()
    assert "error" in data
    # Verifies caller X-Request-ID is preserved (catches miss_4 generating a random UUID)
    assert data["error"].get("request_id") == custom_trace_id, (
        f"Expected request_id '{custom_trace_id}', got '{data['error'].get('request_id')}'"
    )
    assert response.headers.get("x-request-id") == custom_trace_id


def test_account_not_found_returns_404_error_envelope():
    client = TestClient(app)
    response = client.post(
        "/transfers",
        json={"from_account": "acc_missing", "to_account": "acc_2", "amount": 10.0},
    )
    assert response.status_code == 404
    data = response.json()
    assert "error" in data
    assert data["error"].get("code") == "ACCOUNT_NOT_FOUND"
    assert data["error"]["details"].get("account_id") == "acc_missing"


def test_transfer_amount_must_be_positive_422():
    client = TestClient(app)
    res_neg = client.post("/transfers", json={"from_account": "acc_1", "to_account": "acc_2", "amount": -10.0})
    assert res_neg.status_code == 422

    res_zero = client.post("/transfers", json={"from_account": "acc_1", "to_account": "acc_2", "amount": 0.0})
    assert res_zero.status_code == 422
