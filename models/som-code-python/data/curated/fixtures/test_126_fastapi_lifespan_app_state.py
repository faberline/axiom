import pytest
from fastapi.testclient import TestClient

import candidate
from candidate import Pool, app


def test_requests_share_the_lifespan_pool():
    before = len(candidate.POOLS)
    with TestClient(app) as client:
        assert client.get("/count").json() == {"n": 1, "size": 5}
        assert client.get("/count").json() == {"n": 2, "size": 5}
        assert client.get("/health").json() == {"status": "ok", "pool_closed": False}
    assert len(candidate.POOLS) == before + 1


def test_shutdown_closes_the_pool():
    with TestClient(app) as client:
        client.get("/count")
    assert candidate.POOLS[-1].closed is True


def test_each_startup_gets_a_fresh_pool():
    with TestClient(app) as client:
        client.get("/count")
    first = candidate.POOLS[-1]
    with TestClient(app) as client:
        assert client.get("/count").json()["n"] == 1
    assert candidate.POOLS[-1] is not first
    assert first.closed and candidate.POOLS[-1].closed


def test_pool_rules():
    with pytest.raises(ValueError, match="pool size must be at least 1"):
        Pool(0)
    pool = Pool(1)
    pool.close()
    with pytest.raises(RuntimeError, match="pool is closed"):
        pool.query("select 1")
