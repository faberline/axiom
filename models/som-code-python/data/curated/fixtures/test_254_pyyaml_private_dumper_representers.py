from dataclasses import dataclass, field
from enum import Enum, IntEnum
from pathlib import Path

import pytest
import yaml

from candidate import SettingsDumper, dump_settings


class Mode(Enum):
    FAST = "fast"


class Level(IntEnum):
    HIGH = 3


@dataclass
class Storage:
    root: Path
    mode: Mode = Mode.FAST


@dataclass
class Settings:
    name: str
    level: Level
    storage: Storage
    extra: list[Path] = field(default_factory=list)


def make():
    return Settings("svc", Level.HIGH, Storage(Path("/var/data")), [Path("a/b")])


def test_dump_uses_plain_scalars_in_field_order():
    assert dump_settings(make()) == (
        "name: svc\nlevel: 3\nstorage:\n  root: /var/data\n  mode: fast\n"
        "extra:\n- a/b\n"
    )


def test_output_loads_back_with_safe_load():
    loaded = yaml.safe_load(dump_settings(make()))
    assert loaded["storage"] == {"root": "/var/data", "mode": "fast"}
    assert loaded["level"] == 3


def test_concrete_path_subclasses_are_handled():
    assert "root: rel/x" in dump_settings(
        Settings("s", Level.HIGH, Storage(Path("rel/x")))
    )


def test_global_safe_dumper_is_untouched():
    assert SettingsDumper is not yaml.SafeDumper
    with pytest.raises(yaml.representer.RepresenterError):
        yaml.safe_dump({"m": Mode.FAST})
    with pytest.raises(yaml.representer.RepresenterError):
        yaml.safe_dump({"p": Path("x")})


def test_only_dataclass_instances_are_accepted():
    for bad in (Settings, {"name": "svc"}, "svc"):
        with pytest.raises(TypeError, match="settings must be a dataclass instance"):
            dump_settings(bad)
