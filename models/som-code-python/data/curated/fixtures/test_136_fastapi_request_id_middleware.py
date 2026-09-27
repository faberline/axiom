import re

import pytest
from fastapi.testclient import TestClient

import candidate
from candidate import app

client = TestClient(app, raise_server_exceptions=False)


@pytest.fixture(autouse=True)
def fresh():
    candidate.reset_db()


def test_generated_id_is_returned_and_visible_to_handler():
    r = client.get("/whoami")
    rid = r.headers["x-request-id"]
    assert re.fullmatch(r"[0-9a-f]{32}", rid)
    assert r.json() == {"request_id": rid}


def test_valid_incoming_id_is_propagated():
    r = client.get("/whoami", headers={"X-Request-Id": "abc-123_XYZ"})
    assert r.headers["x-request-id"] == "abc-123_XYZ"
    assert r.json() == {"request_id": "abc-123_XYZ"}


def test_unsafe_incoming_id_is_replaced():
    for bad in ["has space", "x" * 65, "<script>"]:
        r = client.get("/whoami", headers={"X-Request-Id": bad})
        assert r.headers["x-request-id"] != bad
        assert re.fullmatch(r"[0-9a-f]{32}", r.headers["x-request-id"])


def test_each_request_gets_its_own_id():
    a = client.get("/whoami").headers["x-request-id"]
    b = client.get("/whoami").headers["x-request-id"]
    assert a != b


def test_log_records_id_status_and_path():
    r = client.get("/whoami", headers={"X-Request-Id": "req-1"})
    assert r.status_code == 200
    assert candidate.LOG == [("req-1", "GET", "/whoami", 200)]


def test_errors_are_logged_as_500_with_the_id():
    r = client.get("/boom", headers={"X-Request-Id": "req-2"})
    assert r.status_code == 500
    assert candidate.LOG == [("req-2", "GET", "/boom", 500)]
