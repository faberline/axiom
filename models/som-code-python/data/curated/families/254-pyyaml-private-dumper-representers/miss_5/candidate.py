"""Dump dataclass settings through representers on a private SafeDumper."""

from __future__ import annotations

import dataclasses
from enum import Enum
from pathlib import PurePath
from typing import Any

import yaml
from yaml.nodes import Node
from yaml.representer import SafeRepresenter


class SettingsDumper(yaml.SafeDumper):  # pylint: disable=too-many-ancestors
    """A SafeDumper that also knows paths and enums."""


def _represent_path(dumper: SafeRepresenter, data: PurePath) -> Node:
    return dumper.represent_str(data.as_posix())


def _represent_enum(dumper: SafeRepresenter, data: Enum) -> Node:
    return dumper.represent_str(str(data.value))


SettingsDumper.add_multi_representer(PurePath, _represent_path)
SettingsDumper.add_multi_representer(Enum, _represent_enum)


def dump_settings(settings: Any) -> str:
    """Render a dataclass instance, nested ones included, as YAML."""
    if not dataclasses.is_dataclass(settings) or isinstance(settings, type):
        raise TypeError("settings must be a dataclass instance")
    return yaml.dump(
        dataclasses.asdict(settings), Dumper=SettingsDumper, sort_keys=False
    )
