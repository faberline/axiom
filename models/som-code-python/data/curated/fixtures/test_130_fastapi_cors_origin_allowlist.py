from fastapi.testclient import TestClient

from candidate import ALLOWED_ORIGINS, app

client = TestClient(app)
GOOD = "https://app.example.com"
EVIL = "https://evil.example.net"


def preflight(origin, method="DELETE"):
    return client.options(
        "/notes/1",
        headers={
            "Origin": origin,
            "Access-Control-Request-Method": method,
            "Access-Control-Request-Headers": "authorization",
        },
    )


def test_allowlist_is_explicit():
    assert "*" not in ALLOWED_ORIGINS
    assert GOOD in ALLOWED_ORIGINS


def test_allowed_origin_preflight():
    r = preflight(GOOD)
    assert r.status_code == 200
    assert r.headers["access-control-allow-origin"] == GOOD
    assert r.headers["access-control-allow-credentials"] == "true"
    assert "DELETE" in r.headers["access-control-allow-methods"]
    assert r.headers["access-control-max-age"] == "3600"


def test_unknown_origin_is_not_echoed():
    r = preflight(EVIL)
    assert r.status_code == 400
    assert "access-control-allow-origin" not in r.headers
    simple = client.get("/notes/1", headers={"Origin": EVIL})
    assert simple.status_code == 200
    assert "access-control-allow-origin" not in simple.headers


def test_unlisted_method_is_refused():
    assert preflight(GOOD, "PUT").status_code == 400


def test_simple_request_exposes_custom_header():
    r = client.get("/notes/1", headers={"Origin": GOOD})
    assert r.json() == {"id": 1, "text": "hello"}
    assert r.headers["access-control-allow-origin"] == GOOD
    assert r.headers["x-note-version"] == "3"
    assert "x-note-version" in r.headers["access-control-expose-headers"].lower()
