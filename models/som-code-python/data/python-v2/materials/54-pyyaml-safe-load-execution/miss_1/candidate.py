from typing import Any, Dict, Tuple
import yaml


class ConfigValidationError(Exception):
    pass


class PipelineManifestParser:
    def __init__(self, required_keys: Tuple[str, ...] = ("version", "pipeline", "steps")):
        if not required_keys:
            raise ValueError("required_keys must not be empty")
        self.required_keys = required_keys

    def parse_manifest(self, content: str) -> Dict[str, Any]:
        if not isinstance(content, str):
            raise TypeError("Manifest content must be a string")
        if not content.strip():
            raise ConfigValidationError("Manifest content must not be empty")
        try:
            data = yaml.load(content, Loader=yaml.Loader)
        except yaml.YAMLError as exc:
            raise ConfigValidationError(f"Invalid YAML syntax: {exc}") from exc

        if not isinstance(data, dict):
            raise ConfigValidationError("Manifest root must be a mapping/dictionary")

        for key in self.required_keys:
            if key not in data:
                raise ConfigValidationError(f"Missing required key: {key}")

        return data


def parse_manifest(yaml_text: str) -> Dict[str, Any]:
    return PipelineManifestParser().parse_manifest(yaml_text)
