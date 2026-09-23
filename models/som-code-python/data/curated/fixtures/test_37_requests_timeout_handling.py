"""Oracle test suite for 37-requests-timeout-handling."""
import pytest
import candidate


def test_default_timeout_is_tuple():
    adapter = candidate.TimeoutHTTPAdapter()
    assert adapter.default_timeout == (3.05, 27.0)


def test_negative_timeout_rejected():
    with pytest.raises(ValueError, match="positive"):
        candidate.TimeoutHTTPAdapter(default_timeout=(-1.0, 5.0))


def test_zero_timeout_rejected():
    with pytest.raises(ValueError, match="positive"):
        candidate.TimeoutHTTPAdapter(default_timeout=(0.0, 5.0))
    with pytest.raises(ValueError, match="positive"):
        candidate.TimeoutHTTPAdapter(default_timeout=0)


def test_custom_timeout_preserved_in_send(monkeypatch):
    adapter = candidate.TimeoutHTTPAdapter(default_timeout=(3.05, 27.0))
    recorded_timeouts = []

    def dummy_send(self, request, stream=False, timeout=None, verify=True, cert=None, proxies=None):
        recorded_timeouts.append(timeout)
        resp = candidate.requests.Response()
        resp.status_code = 200
        return resp

    monkeypatch.setattr(candidate.HTTPAdapter, "send", dummy_send)
    req = candidate.requests.Request("GET", "https://example.com").prepare()

    adapter.send(req, timeout=None)
    assert recorded_timeouts[-1] == (3.05, 27.0)

    adapter.send(req, timeout=12.5)
    assert recorded_timeouts[-1] == 12.5


def test_create_timeout_session_mounts_timeout_adapter():
    session = candidate.create_timeout_session(default_timeout=(5.0, 15.0))
    for scheme in ("http://", "https://"):
        adapter = session.adapters[scheme]
        assert isinstance(adapter, candidate.TimeoutHTTPAdapter)
        assert adapter.default_timeout == (5.0, 15.0)
