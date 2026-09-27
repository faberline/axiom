import pytest
from fastapi.testclient import TestClient

from candidate import app, get_settings, load_settings

client = TestClient(app)


@pytest.fixture
def use_env():
    def apply(env):
        app.dependency_overrides[get_settings] = lambda: load_settings(env)

    yield apply
    app.dependency_overrides.clear()


def test_overridden_settings_drive_the_endpoints(use_env):
    use_env({"APP_ENV": "staging", "BETA_FEATURES": "search, export,"})
    assert client.get("/config").json() == {"environment": "staging", "max_upload_mb": 10}
    assert client.get("/features/search").json() == {"name": "search", "enabled": True}
    assert client.get("/features/chat").json()["enabled"] is False


def test_production_disables_beta_features(use_env):
    use_env({"APP_ENV": "production", "BETA_FEATURES": "search"})
    assert client.get("/features/search").json()["enabled"] is False


def test_defaults_and_invalid_values():
    s = load_settings({})
    assert (s.environment, s.max_upload_mb, s.beta_features) == ("development", 10, frozenset())
    with pytest.raises(ValueError, match="unknown APP_ENV"):
        load_settings({"APP_ENV": "prod"})
    with pytest.raises(ValueError, match="must be an integer"):
        load_settings({"MAX_UPLOAD_MB": "ten"})
    with pytest.raises(ValueError, match="must be at least 1"):
        load_settings({"MAX_UPLOAD_MB": "0"})


def test_real_settings_are_read_once(monkeypatch):
    get_settings.cache_clear()
    try:
        monkeypatch.setenv("APP_ENV", "staging")
        first = get_settings()
        monkeypatch.setenv("APP_ENV", "production")
        assert get_settings() is first
        assert first.environment == "staging"
    finally:
        get_settings.cache_clear()
