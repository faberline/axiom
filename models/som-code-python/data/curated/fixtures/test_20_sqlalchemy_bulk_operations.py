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


def test_bulk_insert_strictly_one_statement(client):
    payload = {
        "readings": [
            {
                "device_id": f"dev-{i:03d}",
                "metric_name": "temperature",
                "value": 20.0 + (i * 0.1),
                "timestamp": f"2026-09-22T00:{i:02d}:00Z",
            }
            for i in range(50)
        ]
    }
    candidate.QueryCounter.reset()
    res = client.post("/telemetry/bulk", json=payload)
    assert res.status_code == 201
    assert candidate.QueryCounter.insert_count == 1


def test_bulk_insert_persists_to_database(client):
    payload = {
        "readings": [
            {
                "device_id": f"dev-{i:03d}",
                "metric_name": "temperature",
                "value": 20.0 + (i * 0.1),
                "timestamp": f"2026-09-22T00:{i:02d}:00Z",
            }
            for i in range(50)
        ]
    }
    res = client.post("/telemetry/bulk", json=payload)
    assert res.status_code == 201

    check = client.get("/telemetry")
    assert check.status_code == 200
    assert check.json()["total"] == 50


def test_bulk_insert_rejects_empty_payload(client):
    res = client.post("/telemetry/bulk", json={"readings": []})
    assert res.status_code == 400
    assert res.json()["detail"] == "Readings payload cannot be empty"


def test_bulk_insert_returns_all_ids(client):
    payload = {
        "readings": [
            {
                "device_id": f"dev-{i:03d}",
                "metric_name": "temperature",
                "value": 20.0 + (i * 0.1),
                "timestamp": f"2026-09-22T00:{i:02d}:00Z",
            }
            for i in range(50)
        ]
    }
    res = client.post("/telemetry/bulk", json=payload)
    assert res.status_code == 201
    data = res.json()
    assert data["inserted_count"] == 50
    assert len(data["ids"]) == 50
    assert data["ids"][0] == 1
    assert data["ids"][-1] == 50
