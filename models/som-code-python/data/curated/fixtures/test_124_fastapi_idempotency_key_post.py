import pytest
from fastapi.testclient import TestClient

import candidate
from candidate import app

client = TestClient(app)
BODY = {"account": "acc-1", "amount_cents": 500}


@pytest.fixture(autouse=True)
def clean_database():
    candidate.reset_db()
    yield


def pay(key, body=BODY):
    headers = {} if key is None else {"Idempotency-Key": key}
    return client.post("/payments", json=body, headers=headers)


def test_first_request_creates_a_payment():
    res = pay("key-00000001")
    assert res.status_code == 201
    assert res.json() == {"id": 1, "account": "acc-1", "amount_cents": 500}
    assert "Idempotent-Replayed" not in res.headers


def test_retry_replays_without_charging_twice():
    first = pay("key-00000001")
    again = pay("key-00000001")
    assert again.status_code == 201
    assert again.json() == first.json()
    assert again.headers["Idempotent-Replayed"] == "true"
    assert len(candidate.PAYMENTS) == 1
    assert pay("key-00000002").json()["id"] == 2


def test_reusing_a_key_with_another_body_conflicts():
    pay("key-00000001")
    res = pay("key-00000001", {"account": "acc-1", "amount_cents": 900})
    assert res.status_code == 409
    assert res.json()["detail"] == "idempotency key reused with a different payload"


def test_key_is_required_and_bounded():
    assert pay(None).status_code == 422
    assert pay("short").status_code == 422
    assert pay("key-00000003", {"account": "acc-1", "amount_cents": 0}).status_code == 422
    assert candidate.PAYMENTS == []
