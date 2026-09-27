import pytest
from fastapi.testclient import TestClient

import candidate
from candidate import app

client = TestClient(app)
ADMIN = {"X-Api-Key": "admin-key-127"}
VIEWER = {"X-Api-Key": "viewer-key-127"}


@pytest.fixture(autouse=True)
def fresh():
    candidate.reset_db()


def test_public_route_needs_no_key():
    assert client.get("/public/ping").json() == {"pong": True}


def test_every_admin_route_requires_a_key():
    assert client.get("/admin/users").status_code == 401
    assert client.delete("/admin/users/1").status_code == 401
    r = client.get("/admin/users", headers={"X-Api-Key": "nope"})
    assert r.status_code == 401
    assert r.json() == {"detail": "invalid api key"}


def test_non_admin_role_is_forbidden():
    r = client.get("/admin/users", headers=VIEWER)
    assert r.status_code == 403
    assert r.json() == {"detail": "admin role required"}


def test_admin_can_list_and_delete():
    assert client.get("/admin/users", headers=ADMIN).json() == ["ada", "bob"]
    assert client.delete("/admin/users/1", headers=ADMIN).status_code == 204
    assert client.get("/admin/users", headers=ADMIN).json() == ["ada"]
    r = client.delete("/admin/users/9", headers=ADMIN)
    assert r.status_code == 404
    assert r.json() == {"detail": "user 9 not found"}


def test_audit_log_records_admin_calls_only():
    client.get("/public/ping")
    client.get("/admin/users", headers=ADMIN)
    client.get("/admin/users", headers=VIEWER)
    assert candidate.AUDIT == ["admin:GET /admin/users"]
