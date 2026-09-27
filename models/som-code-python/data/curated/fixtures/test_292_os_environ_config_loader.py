import pytest

from candidate import ConfigError, Settings, load

DB = {"APP_DATABASE_URL": "postgres://db/app"}


def test_defaults_and_blank_values():
    settings = load({**DB, "APP_PORT": "  ", "APP_DEBUG": ""})
    assert settings == Settings("postgres://db/app", 8000, False, 1)


def test_parsing_and_prefix():
    env = {
        "SVC_DATABASE_URL": " sqlite:///x ",
        "SVC_PORT": "65535",
        "SVC_DEBUG": "YES",
        "SVC_WORKERS": "64",
    }
    assert load(env, prefix="SVC_") == Settings("sqlite:///x", 65535, True, 64)
    assert load({**DB, "APP_DEBUG": "Off", "APP_PORT": "1"}).port == 1


def test_os_environ_is_the_default_but_an_empty_mapping_is_respected(monkeypatch):
    monkeypatch.setenv("APP_DATABASE_URL", "postgres://env/app")
    assert load().database_url == "postgres://env/app"
    with pytest.raises(ConfigError, match="APP_DATABASE_URL is required"):
        load({})


def test_invalid_values_raise():
    for key, value in [
        ("APP_PORT", "0"),
        ("APP_PORT", "65536"),
        ("APP_WORKERS", "65"),
        ("APP_DEBUG", "maybe"),
    ]:
        with pytest.raises(ConfigError, match=key):
            load({**DB, key: value})
    assert issubclass(ConfigError, ValueError)


def test_errors_do_not_leak_the_raw_value():
    with pytest.raises(ConfigError) as info:
        load({**DB, "APP_PORT": "s3cret"})
    assert "s3cret" not in str(info.value)
    assert info.value.__cause__ is None
    assert info.value.__suppress_context__
