import os
import pytest
import yaml
from candidate import (
    ApplicationConfigParser,
    ConfigParseError,
    SafeAppConfigLoader,
    create_secure_yaml_loader,
    load_custom_config,
)


@pytest.fixture
def parser():
    return ApplicationConfigParser()


def test_valid_env_interpolation(parser, monkeypatch):
    monkeypatch.setenv("APP_PORT", "9090")
    config_yaml = """
server:
  host: !env SERVER_HOST:localhost
  port: !env APP_PORT:8080
"""
    data = parser.parse_config(config_yaml)
    assert data["server"]["host"] == "localhost"
    assert data["server"]["port"] == "9090"


def test_helpers():
    loader_cls = create_secure_yaml_loader()
    assert issubclass(loader_cls, yaml.SafeLoader)
    data = load_custom_config("server: {port: 3000}")
    assert data["server"]["port"] == 3000


def test_arbitrary_code_execution_blocked(parser):
    exploit = """
server:
  hook: !env APP_PORT:8080
  exploit: !!python/object/apply:os.system ['echo COMPROMISED']
"""
    with pytest.raises(ConfigParseError):
        parser.parse_config(exploit)


def test_global_safeloader_not_polluted():
    assert "!env" not in yaml.SafeLoader.yaml_constructors, (
        "Global yaml.SafeLoader was polluted with custom !env constructor"
    )


def test_env_fallback_used_when_var_missing(parser, monkeypatch):
    monkeypatch.delenv("MISSING_VAR_XYZ", raising=False)
    config_yaml = "timeout: !env MISSING_VAR_XYZ:60"
    data = parser.parse_config(config_yaml)
    assert data["timeout"] == "60"


def test_missing_env_without_default_raises_error(parser, monkeypatch):
    monkeypatch.delenv("UNDEFINED_SECRET_KEY", raising=False)
    config_yaml = "api_key: !env UNDEFINED_SECRET_KEY"
    with pytest.raises(ConfigParseError) as exc_info:
        parser.parse_config(config_yaml)
    assert "UNDEFINED_SECRET_KEY" in str(exc_info.value)


def test_empty_var_name_boundary_rejected(parser):
    with pytest.raises(ConfigParseError, match="Variable name cannot be empty"):
        parser.parse_config("key: !env :default_value")


def test_scalar_root_rejected(parser):
    with pytest.raises(ConfigParseError):
        parser.parse_config("a plain string")
