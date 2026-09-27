import pytest
from fastapi.testclient import TestClient

import candidate
from candidate import LIMIT, WINDOW, FixedWindow, app

client = TestClient(app)


@pytest.fixture(autouse=True)
def fresh():
    candidate.reset_db()
    candidate.NOW[0] = 1000.0


def call(key="k1"):
    return client.get("/search", headers={"X-Client-Id": key})


def test_limit_requests_pass_with_remaining_header():
    for i in range(LIMIT):
        r = call()
        assert r.status_code == 200
        assert r.headers["x-ratelimit-remaining"] == str(LIMIT - i - 1)


def test_next_request_is_429_with_retry_after():
    for _ in range(LIMIT):
        call()
    candidate.NOW[0] += 12.2
    r = call()
    assert r.status_code == 429
    assert r.json() == {"detail": "rate limit exceeded"}
    assert r.headers["retry-after"] == str(WINDOW - 12)


def test_window_resets():
    for _ in range(LIMIT):
        call()
    candidate.NOW[0] += WINDOW
    assert call().status_code == 200


def test_clients_are_counted_separately():
    for _ in range(LIMIT):
        call("a")
    assert call("b").status_code == 200
    assert call("a").status_code == 429


def test_missing_client_id_is_400():
    r = client.get("/search")
    assert r.status_code == 400
    assert r.json() == {"detail": "X-Client-Id header required"}


def test_limiter_rejects_bad_settings():
    with pytest.raises(ValueError, match="limit and window must be positive"):
        FixedWindow(0, 60)
