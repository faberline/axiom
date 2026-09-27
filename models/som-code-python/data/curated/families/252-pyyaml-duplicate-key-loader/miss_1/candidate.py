"""Load YAML with a SafeLoader subclass that rejects duplicate mapping keys."""

from __future__ import annotations

from typing import Any

import yaml
from yaml.constructor import ConstructorError
from yaml.nodes import MappingNode

MERGE_TAG = "tag:yaml.org,2002:merge"


class ConfigError(Exception):
    """Raised with a one-based line number when the YAML is rejected."""

    def __init__(self, message: str, line: int) -> None:
        super().__init__(f"line {line}: {message}")
        self.line = line


class UniqueKeyLoader(yaml.SafeLoader):  # pylint: disable=too-many-ancestors
    """A SafeLoader that refuses a mapping which repeats a key."""

    def construct_mapping(
        self, node: MappingNode, deep: bool = False
    ) -> dict[Any, Any]:
        seen: set[Any] = set()
        for key_node, _ in node.value:
            if key_node.tag == MERGE_TAG:
                continue
            key = self.construct_object(key_node, deep=deep)
            if key in seen:
                raise ConstructorError(
                    "while constructing a mapping",
                    node.start_mark,
                    f"found duplicate key {key!r}",
                    key_node.start_mark,
                )
        return super().construct_mapping(node, deep=deep)


def load_strict(text: str) -> Any:
    """Parse text, raising ConfigError for duplicates or invalid YAML."""
    try:
        return yaml.load(text, Loader=UniqueKeyLoader)
    except yaml.MarkedYAMLError as exc:
        mark = exc.problem_mark
        line = mark.line + 1 if mark is not None else 0
        raise ConfigError(str(exc.problem), line) from exc
