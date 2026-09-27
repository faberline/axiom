import os

import pytest

from candidate import Settings, app_env, load_settings

__all__ = ["app_env"]


@pytest.fixture
def stray(monkeypatch):
    monkeypatch.setenv("APP_PORT", "9999")
    monkeypatch.setenv("APP_DEBUG", "yes")


def test_defaults_ignore_stray_variables(stray, app_env):
    assert load_settings() == Settings(port=8000, debug=False, hosts=("localhost",))
    assert not [name for name in os.environ if name.startswith("APP_")]


def test_values_are_parsed(app_env):
    app_env.setenv("APP_PORT", "65535")
    app_env.setenv("APP_DEBUG", " TRUE ")
    app_env.setenv("APP_HOSTS", "a.example, b.example,,")
    assert load_settings() == Settings(
        port=65535, debug=True, hosts=("a.example", "b.example")
    )


@pytest.mark.parametrize(
    ("name", "value", "message"),
    [
        ("APP_PORT", "http", "APP_PORT must be an integer, got 'http'"),
        ("APP_PORT", "0", "APP_PORT must be between 1 and 65535"),
        ("APP_PORT", "65536", "APP_PORT must be between 1 and 65535"),
        ("APP_DEBUG", "maybe", "APP_DEBUG must be a boolean, got 'maybe'"),
    ],
)
def test_invalid_values(app_env, name, value, message):
    app_env.setenv(name, value)
    with pytest.raises(ValueError, match=message):
        load_settings()


def test_settings_are_read_at_call_time(app_env):
    app_env.setenv("APP_PORT", "1")
    assert load_settings().port == 1
    app_env.setenv("APP_PORT", "2")
    assert load_settings().port == 2
