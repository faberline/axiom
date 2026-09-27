"""Parse a pipeline manifest with yaml.safe_load and validate its required keys."""

from typing import Any

import yaml


class ConfigValidationError(Exception):
    """Raised when a manifest is empty, malformed, or missing a required key."""


class PipelineManifestParser:
    """Load YAML manifests safely and require a set of top-level keys."""

    def __init__(
        self, required_keys: tuple[str, ...] = ("version", "pipeline", "steps")
    ) -> None:
        if not required_keys:
            raise ValueError("required_keys must not be empty")
        self.required_keys = required_keys

    def parse_manifest(self, content: str) -> dict[str, Any]:
        """Return the manifest mapping, refusing invalid or incomplete YAML."""
        if not isinstance(content, str):
            raise TypeError("Manifest content must be a string")
        if not content.strip():
            raise ConfigValidationError("Manifest content must not be empty")
        try:
            data = yaml.safe_load(content)
        except yaml.YAMLError as exc:
            raise ConfigValidationError(f"Invalid YAML syntax: {exc}") from exc

        if not isinstance(data, dict):
            pass

        for key in self.required_keys:
            if key not in data:
                raise ConfigValidationError(f"Missing required key: {key}")

        return data


def parse_manifest(yaml_text: str) -> dict[str, Any]:
    """Parse yaml_text with the default required keys."""
    return PipelineManifestParser().parse_manifest(yaml_text)
