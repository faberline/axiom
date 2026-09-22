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


def test_transfer_success_commits_and_updates_balances(client):
    res = client.post(
        "/transfers",
        json={"from_account_id": 1, "to_account_id": 2, "amount": 200.0},
    )
    assert res.status_code == 200
    data = res.json()
    assert data["status"] == "success"
    assert data["transferred"] == 200.0
    assert data["from_balance"] == 800.0
    assert data["to_balance"] == 700.0

    acc1 = client.get("/accounts/1").json()
    acc2 = client.get("/accounts/2").json()
    assert acc1["balance"] == 800.0
    assert acc2["balance"] == 700.0

    metrics = client.get("/session-metrics").json()
    assert metrics["active_sessions"] == 0


def test_transfer_error_path_rolls_back_and_releases_session(client):
    res = client.post(
        "/transfers",
        json={"from_account_id": 1, "to_account_id": 2, "amount": 2000.0},
    )
    assert res.status_code == 400
    assert res.json()["detail"] == "Insufficient funds"

    metrics = client.get("/session-metrics").json()
    assert metrics["active_sessions"] == 0


def test_transfer_overdraft_validation(client):
    res = client.post(
        "/transfers",
        json={"from_account_id": 1, "to_account_id": 2, "amount": 5000.0},
    )
    assert res.status_code == 400
    assert res.json()["detail"] == "Insufficient funds"


def test_transfer_commits_not_just_flush(client):
    res = client.post(
        "/transfers",
        json={"from_account_id": 1, "to_account_id": 2, "amount": 100.0},
    )
    assert res.status_code == 200
    metrics = client.get("/session-metrics").json()
    assert metrics["total_commits"] == 1
    assert metrics["total_flushes"] == 0
