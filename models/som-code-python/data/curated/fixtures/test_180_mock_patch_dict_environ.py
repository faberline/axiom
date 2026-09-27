"""Fixture for 180: environment-driven config isolated with patch.dict."""

import os

import pytest

from candidate import ServiceConfig, isolated_env, load_config


def test_reads_every_setting() -> None:
    with isolated_env(
        SERVICE_URL=" http://svc ", SERVICE_WORKERS="8", SERVICE_DEBUG="Yes"
    ):
        assert load_config() == ServiceConfig("http://svc", 8, True)


def test_defaults_apply_when_optional_settings_are_absent() -> None:
    with isolated_env(SERVICE_URL="http://svc"):
        assert load_config() == ServiceConfig("http://svc", 4, False)


@pytest.mark.parametrize("flag", ["0", "no", "", "off"])
def test_other_debug_values_are_false(flag: str) -> None:
    with isolated_env(SERVICE_URL="http://svc", SERVICE_DEBUG=flag):
        assert load_config().debug is False


def test_outer_variables_do_not_leak_in(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("SERVICE_URL", "http://leaked")
    with isolated_env(), pytest.raises(LookupError, match="SERVICE_URL"):
        load_config()


def test_blank_url_is_missing() -> None:
    with isolated_env(SERVICE_URL="   "), pytest.raises(LookupError):
        load_config()


def test_workers_must_be_positive() -> None:
    with (
        isolated_env(SERVICE_URL="http://svc", SERVICE_WORKERS="0"),
        pytest.raises(ValueError, match="SERVICE_WORKERS must be at least 1, got 0"),
    ):
        load_config()


def test_environment_is_restored(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("SERVICE_URL", "http://outer")
    monkeypatch.delenv("SERVICE_WORKERS", raising=False)
    with isolated_env(SERVICE_URL="http://inner", SERVICE_WORKERS="2"):
        assert load_config().url == "http://inner"
    assert os.environ["SERVICE_URL"] == "http://outer"
    assert "SERVICE_WORKERS" not in os.environ
