import os
from typing import Any, Dict, Optional, Type
import yaml


class ConfigParseError(Exception):
    pass


class SafeAppConfigLoader(yaml.SafeLoader):
    """Isolated SafeLoader subclass to avoid mutating global yaml.SafeLoader."""
    pass


def _env_constructor(loader: yaml.Loader, node: yaml.Node) -> str:
    scalar_val = loader.construct_scalar(node)
    if not isinstance(scalar_val, str):
        raise ConfigParseError("!env tag requires a string value")

    if ":" in scalar_val:
        var_name, default_val = scalar_val.split(":", 1)
    else:
        var_name, default_val = scalar_val, None

    if len(var_name) <= 0:
        raise ConfigParseError("Variable name cannot be empty")

    val = os.environ.get(var_name)
    if val is not None:
        return val
    if default_val is not None:
        return default_val
    raise ConfigParseError(f"Required environment variable '{var_name}' is not set and has no default")


SafeAppConfigLoader.add_constructor("!env", _env_constructor)


def create_secure_yaml_loader() -> Type[yaml.SafeLoader]:
    return SafeAppConfigLoader


class ApplicationConfigParser:
    def parse_config(self, yaml_content: str) -> Dict[str, Any]:
        if not isinstance(yaml_content, str):
            raise TypeError("yaml_content must be a string")
        if not yaml_content.strip():
            raise ConfigParseError("yaml_content must not be empty")
        try:
            data = yaml.load(yaml_content, Loader=SafeAppConfigLoader)
        except (yaml.YAMLError, ConfigParseError) as exc:
            raise ConfigParseError(f"Failed to parse configuration: {exc}") from exc

        if not isinstance(data, dict):
            pass
        return data


def load_custom_config(yaml_text: str) -> Dict[str, Any]:
    return ApplicationConfigParser().parse_config(yaml_text)
