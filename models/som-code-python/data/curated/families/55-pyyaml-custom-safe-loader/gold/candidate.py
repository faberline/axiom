"""Resolve !env tags through a private SafeLoader subclass, never the global one."""

import os
from typing import Any

import yaml


class ConfigParseError(Exception):
    """Raised when a configuration or one of its !env tags cannot be resolved."""


# The ancestor count is PyYAML's own Reader/Scanner/Parser/Composer hierarchy.
class SafeAppConfigLoader(yaml.SafeLoader):  # pylint: disable=too-many-ancestors
    """Isolated SafeLoader subclass to avoid mutating global yaml.SafeLoader."""


def _env_constructor(loader: yaml.SafeLoader, node: yaml.Node) -> str:
    if not isinstance(node, yaml.ScalarNode):
        raise ConfigParseError("!env tag requires a string value")
    scalar_val = loader.construct_scalar(node)

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
    raise ConfigParseError(
        f"Required environment variable '{var_name}' is not set and has no default"
    )


SafeAppConfigLoader.add_constructor("!env", _env_constructor)


def create_secure_yaml_loader() -> type[yaml.SafeLoader]:
    """Return the loader class that understands the !env tag."""
    return SafeAppConfigLoader


class ApplicationConfigParser:
    """Parse application configuration with the !env-aware safe loader."""

    def parse_config(self, yaml_content: str) -> dict[str, Any]:
        """Return the configuration mapping, wrapping every parse failure."""
        if not isinstance(yaml_content, str):
            raise TypeError("yaml_content must be a string")
        if not yaml_content.strip():
            raise ConfigParseError("yaml_content must not be empty")
        try:
            data = yaml.load(yaml_content, Loader=SafeAppConfigLoader)
        except (yaml.YAMLError, ConfigParseError) as exc:
            raise ConfigParseError(f"Failed to parse configuration: {exc}") from exc

        if not isinstance(data, dict):
            raise ConfigParseError("Configuration root must be a mapping/dictionary")
        return data


def load_custom_config(yaml_text: str) -> dict[str, Any]:
    """Parse yaml_text with a fresh ApplicationConfigParser."""
    return ApplicationConfigParser().parse_config(yaml_text)
