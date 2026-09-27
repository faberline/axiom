import pytest
from fastapi.testclient import TestClient

import candidate
from candidate import app

client = TestClient(app)


@pytest.fixture(autouse=True)
def fresh():
    candidate.reset_db()


def test_get_returns_version_etag():
    r = client.get("/docs/1")
    assert r.json() == {"title": "Draft", "version": 1}
    assert r.headers["etag"] == '"v1"'


def test_update_with_current_etag():
    r = client.put("/docs/1", json={"title": "Final"}, headers={"If-Match": '"v1"'})
    assert r.status_code == 200
    assert r.json() == {"title": "Final", "version": 2}
    assert r.headers["etag"] == '"v2"'


def test_stale_etag_is_412_and_nothing_changes():
    client.put("/docs/1", json={"title": "A"}, headers={"If-Match": '"v1"'})
    r = client.put("/docs/1", json={"title": "B"}, headers={"If-Match": '"v1"'})
    assert r.status_code == 412
    assert r.json() == {"detail": "document changed; current version is 2"}
    assert client.get("/docs/1").json()["title"] == "A"


def test_missing_if_match_is_428():
    r = client.put("/docs/1", json={"title": "X"})
    assert r.status_code == 428
    assert r.json() == {"detail": "If-Match header required"}


def test_weak_tags_are_refused():
    r = client.put("/docs/1", json={"title": "X"}, headers={"If-Match": 'W/"v1"'})
    assert r.status_code == 412


def test_unknown_document_is_404():
    r = client.put("/docs/9", json={"title": "X"}, headers={"If-Match": '"v1"'})
    assert r.status_code == 404
