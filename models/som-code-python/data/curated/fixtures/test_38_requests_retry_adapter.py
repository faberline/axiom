"""Oracle test suite for 38-requests-retry-adapter."""
import pytest
from urllib3.util import Retry
import candidate


def test_default_allowed_methods_excludes_post():
    strategy = candidate.build_retry_strategy()
    assert "POST" not in strategy.allowed_methods
    assert "GET" in strategy.allowed_methods
    assert "HEAD" in strategy.allowed_methods


def test_negative_backoff_factor_rejected():
    with pytest.raises(ValueError, match="backoff_factor"):
        candidate.build_retry_strategy(backoff_factor=-0.5)


def test_zero_total_retries_allowed():
    strategy = candidate.build_retry_strategy(total=0)
    assert strategy.total == 0


def test_retry_adapter_mounted_both_schemes():
    session = candidate.create_retry_session(total=2)
    assert "http://" in session.adapters
    assert "https://" in session.adapters
    http_adapter = session.adapters["http://"]
    https_adapter = session.adapters["https://"]
    assert isinstance(http_adapter.max_retries, Retry)
    assert isinstance(https_adapter.max_retries, Retry)
    assert http_adapter.max_retries.total == 2
    assert https_adapter.max_retries.total == 2


def test_adapter_uses_configured_retry_strategy():
    session = candidate.create_retry_session(backoff_factor=1.5, status_forcelist=(503, 504))
    adapter = session.adapters["http://"]
    assert isinstance(adapter.max_retries, Retry)
    assert adapter.max_retries.backoff_factor == 1.5
    assert 503 in adapter.max_retries.status_forcelist
