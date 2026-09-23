"""Oracle test suite for 36-requests-connection-pooling."""
import pytest
import candidate


def test_default_pool_configuration():
    mgr = candidate.PooledSessionManager()
    assert mgr.pool_connections == 10
    assert mgr.pool_maxsize == 20
    assert mgr.pool_block is True
    session = mgr.get_session()
    for scheme in ("http://", "https://"):
        adapter = session.adapters[scheme]
        assert getattr(adapter, "_pool_connections", None) == 10
        assert getattr(adapter, "_pool_maxsize", None) == 20
        assert getattr(adapter, "_pool_block", None) is True
    mgr.close()


def test_pool_maxsize_less_than_connections_validation():
    with pytest.raises(ValueError, match="pool_maxsize"):
        candidate.PooledSessionManager(pool_connections=20, pool_maxsize=10)


def test_zero_and_negative_pool_connections_rejected():
    with pytest.raises(ValueError):
        candidate.PooledSessionManager(pool_connections=0)
    with pytest.raises(ValueError):
        candidate.PooledSessionManager(pool_connections=-5)


def test_adapters_mounted_on_both_http_and_https():
    mgr = candidate.PooledSessionManager(pool_connections=5, pool_maxsize=15)
    session = mgr.get_session()
    assert "http://" in session.adapters
    assert "https://" in session.adapters
    http_adapter = session.adapters["http://"]
    https_adapter = session.adapters["https://"]
    assert getattr(http_adapter, "_pool_connections", None) == 5
    assert getattr(http_adapter, "_pool_maxsize", None) == 15
    assert getattr(https_adapter, "_pool_connections", None) == 5
    assert getattr(https_adapter, "_pool_maxsize", None) == 15
    mgr.close()


def test_session_close_and_context_manager_cleanup():
    mgr = candidate.PooledSessionManager()
    session = mgr.get_session()
    close_called = []
    session.close = lambda: close_called.append(True)
    mgr.close()
    assert len(close_called) == 1, "session.close() must be explicitly called on close()"
    assert mgr._session is None

    with candidate.PooledSessionManager() as s:
        assert s is not None
