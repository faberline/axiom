from fastapi.testclient import TestClient

from candidate import app

client = TestClient(app)


def bearer(token):
    return {"Authorization": f"Bearer {token}"}


def test_reader_can_read():
    r = client.get("/items", headers=bearer("reader-token"))
    assert r.status_code == 200
    assert r.json() == {"user": "rita", "items": ["a", "b"]}


def test_missing_token_is_401_with_challenge():
    r = client.get("/items")
    assert r.status_code == 401
    assert r.headers["www-authenticate"] == 'Bearer scope="items:read"'


def test_unknown_token_is_401():
    r = client.get("/items", headers=bearer("forged"))
    assert r.status_code == 401
    assert r.json() == {"detail": "invalid token"}
    assert r.headers["www-authenticate"] == 'Bearer scope="items:read"'


def test_missing_scope_is_403_with_insufficient_scope():
    r = client.delete("/items/a", headers=bearer("reader-token"))
    assert r.status_code == 403
    assert r.json() == {"detail": "missing scope items:write"}
    assert r.headers["www-authenticate"] == (
        'Bearer error="insufficient_scope", scope="items:read items:write"'
    )


def test_writer_needs_both_scopes_and_gets_them():
    r = client.delete("/items/a", headers=bearer("writer-token"))
    assert r.status_code == 200
    assert r.json() == {"deleted": "a", "by": "wes"}


def test_write_only_token_cannot_read():
    r = client.get("/items", headers=bearer("write-only-token"))
    assert r.status_code == 403
