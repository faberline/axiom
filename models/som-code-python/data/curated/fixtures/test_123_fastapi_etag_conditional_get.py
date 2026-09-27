import pytest
from fastapi.testclient import TestClient

import candidate
from candidate import app

client = TestClient(app)


@pytest.fixture(autouse=True)
def clean_database():
    candidate.reset_db()
    yield


def test_get_sends_a_quoted_etag():
    res = client.get("/articles/1")
    assert res.status_code == 200
    assert res.json()["title"] == "Hello"
    tag = res.headers["ETag"]
    assert tag.startswith('"') and tag.endswith('"') and len(tag) == 18
    assert res.headers["Cache-Control"] == "no-cache"
    assert client.get("/articles/9").status_code == 404


def test_matching_tag_returns_304_with_the_tag():
    tag = client.get("/articles/1").headers["ETag"]
    res = client.get("/articles/1", headers={"If-None-Match": tag})
    assert res.status_code == 304
    assert res.content == b""
    assert res.headers["ETag"] == tag


def test_lists_wildcards_and_weak_tags_match():
    tag = client.get("/articles/1").headers["ETag"]
    for header in (f'"other", {tag}', "*", f"W/{tag}"):
        assert client.get("/articles/1", headers={"If-None-Match": header}).status_code == 304
    assert client.get("/articles/1", headers={"If-None-Match": '"other"'}).status_code == 200


def test_update_changes_the_tag():
    old = client.get("/articles/1").headers["ETag"]
    put = client.put("/articles/1", json={"id": 1, "title": "Hello", "body": "Edited"})
    assert put.status_code == 200
    res = client.get("/articles/1", headers={"If-None-Match": old})
    assert res.status_code == 200
    assert res.headers["ETag"] != old
