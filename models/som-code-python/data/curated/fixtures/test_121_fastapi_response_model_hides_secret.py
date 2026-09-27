import pytest
from fastapi.testclient import TestClient

import candidate
from candidate import app

client = TestClient(app)


@pytest.fixture(autouse=True)
def clean_database():
    candidate.reset_db()
    yield


def signup(email, password="correct horse"):
    return client.post("/users", json={"email": email, "password": password})


def test_signup_returns_201_with_public_fields_only():
    res = signup(" Ann@Example.com ")
    assert res.status_code == 201
    assert res.json() == {"id": 1, "email": "ann@example.com"}


def test_lookup_never_exposes_the_hash():
    signup("ann@example.com")
    res = client.get("/users/1")
    assert res.status_code == 200
    assert res.json() == {"id": 1, "email": "ann@example.com"}
    assert client.get("/users/2").status_code == 404


def test_hashes_are_salted_and_never_the_password():
    signup("ann@example.com")
    signup("bob@example.com")
    first, second = candidate.USERS[1].password_hash, candidate.USERS[2].password_hash
    assert "correct horse" not in first
    assert "$" in first
    assert first != second


def test_duplicate_email_and_short_password_are_rejected():
    signup("ann@example.com")
    dup = signup("ANN@example.com")
    assert dup.status_code == 409
    assert dup.json()["detail"] == "email already registered"
    assert signup("cat@example.com", "seven77").status_code == 422
