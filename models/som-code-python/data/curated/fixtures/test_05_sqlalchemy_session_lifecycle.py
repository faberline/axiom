import candidate
from candidate import app
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
    return TestClient(app)


def test_session_closed_on_success(client):
    metrics_before = client.get("/session-metrics").json()
    create_res = client.post("/items", json={"name": "Alpha"})
    assert create_res.status_code == 201
    metrics_after = client.get("/session-metrics").json()
    assert metrics_after["closed"] > metrics_before["closed"]


def test_session_closed_on_http_exception(client):
    metrics_before = client.get("/session-metrics").json()
    error_res = client.get("/error-http")
    assert error_res.status_code == 400
    metrics_after = client.get("/session-metrics").json()
    assert metrics_after["closed"] > metrics_before["closed"]


def test_session_isolation_discards_uncommitted_state(client):
    leak_res = client.post("/uncommitted-leak")
    assert leak_res.status_code == 400

    list_res = client.get("/items")
    assert list_res.status_code == 200
    items = list_res.json()
    assert not any(item["name"] == "Leaked Item" for item in items)


def test_http_exception_propagates_correct_status_code(client):
    error_res = client.get("/error-http")
    assert error_res.status_code == 400
    assert error_res.json()["detail"] == "Custom HTTP Error"


def test_validation_failure_prevents_persistence(client):
    val_res = client.post("/items-validated", json={"name": "FAIL_TRIGGER"})
    assert val_res.status_code == 422

    list_res = client.get("/items")
    assert list_res.status_code == 200
    items = list_res.json()
    assert not any(item["name"] == "FAIL_TRIGGER" for item in items)
