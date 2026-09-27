from fastapi.testclient import TestClient

from candidate import app

client = TestClient(app)


def test_valid_signup_passes():
    r = client.post("/signup", json={"email": "a@b.co", "age": 30})
    assert r.status_code == 201
    assert r.json() == {"email": "a@b.co", "age": 30}


def test_body_errors_use_the_envelope():
    r = client.post("/signup", json={"email": "x", "age": 5})
    assert r.status_code == 400
    body = r.json()
    assert body["code"] == "validation_failed"
    fields = [e["field"] for e in body["errors"]]
    assert fields == ["email", "age"]
    assert all(isinstance(e["message"], str) and e["message"] for e in body["errors"])


def test_missing_field_is_reported_by_name():
    r = client.post("/signup", json={"email": "a@b.co"})
    assert r.status_code == 400
    assert r.json()["errors"] == [{"field": "age", "message": "Field required"}]


def test_query_errors_keep_their_location():
    r = client.get("/users", params={"limit": "lots"})
    assert r.status_code == 400
    [error] = r.json()["errors"]
    assert error["field"] == "query.limit"


def test_input_values_are_never_echoed():
    r = client.post("/signup", json={"email": "secret-value-132", "age": 5})
    assert "secret-value-132" not in r.text
