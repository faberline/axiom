"""The layer-record validator refuses what docs/reference/layer-records.md does not allow."""

from __future__ import annotations

import copy
from typing import Any

import pytest

from som_core.records import (
    RecordError,
    ScopeTooLarge,
    coverage,
    validate_ops,
    validate_plan,
    validate_topology,
)

PLAN = {"intent": "Create an item over HTTP", "target": "Item", "constraints": ["Return 201"]}

TOPOLOGY: dict[str, Any] = {
    "files": [
        {
            "path": "models.py",
            "blocks": [{"name": "Item", "kind": "class", "depends_on": []}],
        },
        {
            "path": "main.py",
            "blocks": [
                {"name": "app", "kind": "constant", "depends_on": []},
                {"name": "create", "kind": "route", "depends_on": ["app", "models.py:Item"]},
            ],
        },
    ]
}

OPS: list[dict[str, Any]] = [
    {"op": "CREATE_FILE", "path": "models.py", "docstring": "Models."},
    {"op": "ADD_IMPORT", "path": "models.py", "stmt": "from dataclasses import dataclass", "group": 0},
    {"op": "INSERT_BLOCK", "path": "models.py", "block": "Item", "source": "@dataclass\nclass Item:\n    name: str", "blank_before": 2},
    {"op": "CREATE_FILE", "path": "main.py", "docstring": None},
    {"op": "ADD_IMPORT", "path": "main.py", "stmt": "from fastapi import FastAPI", "group": 0},
    {"op": "ADD_IMPORT", "path": "main.py", "stmt": "from models import Item", "group": 1},
    {"op": "INSERT_SNIPPET", "path": "main.py", "block": "app", "id": "fastapi_init", "params": {"app_name": "app"}, "blank_before": 2},
    {"op": "INSERT_BLOCK", "path": "main.py", "block": "create", "source": "def create() -> Item:\n    return Item('x')", "blank_before": 2},
]


def test_a_well_formed_record_set_validates() -> None:
    validate_plan(PLAN)
    validate_topology(TOPOLOGY)
    validate_ops(OPS, TOPOLOGY)
    assert coverage(OPS) == {"snippet_blocks": 1, "literal_blocks": 2}


@pytest.mark.parametrize(
    ("plan", "message"),
    [
        ({"intent": "x", "target": "y"}, "fields must be"),
        ({**PLAN, "constraints": []}, "non-empty list"),
        ({**PLAN, "intent": "  "}, "plan.intent"),
        ({**PLAN, "constraints": ["ok", ""]}, r"plan.constraints\[1\]"),
    ],
)
def test_a_malformed_plan_is_refused(plan: dict[str, Any], message: str) -> None:
    with pytest.raises(RecordError, match=message):
        validate_plan(plan)


def test_more_than_five_files_is_scope_too_large() -> None:
    topo = {"files": [{"path": f"m{i}.py", "blocks": []} for i in range(6)]}
    with pytest.raises(ScopeTooLarge, match="SCOPE_TOO_LARGE: 6 files"):
        validate_topology(topo)


def test_more_than_twenty_blocks_is_scope_too_large() -> None:
    blocks = [{"name": f"b{i}", "kind": "function", "depends_on": []} for i in range(21)]
    with pytest.raises(ScopeTooLarge, match="21 blocks"):
        validate_topology({"files": [{"path": "m.py", "blocks": blocks}]})


def test_exactly_at_the_limit_is_in_scope() -> None:
    per_file = [[{"name": f"b{i}", "kind": "function", "depends_on": []} for i in range(4)] for _ in range(5)]
    validate_topology({"files": [{"path": f"m{j}.py", "blocks": b} for j, b in enumerate(per_file)]})


@pytest.mark.parametrize(
    ("mutate", "message"),
    [
        (lambda t: t["files"][1]["blocks"][1]["depends_on"].append("ghost"), "undeclared block 'ghost'"),
        (lambda t: t["files"][1]["blocks"][1]["depends_on"].append("other.py:Item"), "undeclared block 'other.py:Item'"),
        (lambda t: t["files"][1]["blocks"].append(dict(t["files"][1]["blocks"][0])), "declared twice"),
        (lambda t: t["files"][0]["blocks"][0].update(kind="widget"), "kind"),
        (lambda t: t["files"][0].update(path="../escape.py"), "relative .py path"),
        (lambda t: t["files"][0]["blocks"][0].pop("depends_on"), "fields must be"),
    ],
)
def test_a_malformed_topology_is_refused(mutate: Any, message: str) -> None:
    topo = copy.deepcopy(TOPOLOGY)
    mutate(topo)
    with pytest.raises(RecordError, match=message):
        validate_topology(topo)


def _ops_with(mutate: Any) -> list[dict[str, Any]]:
    ops = copy.deepcopy(OPS)
    mutate(ops)
    return ops


@pytest.mark.parametrize(
    ("mutate", "message"),
    [
        (lambda o: o[2].update(op="INSERT_TEXT"), "unknown operation"),
        (lambda o: o[2].update(extra=1), "fields must be"),
        (lambda o: o[2].update(path="nowhere.py"), "undeclared file"),
        (lambda o: o[2].update(block="Ghost"), "undeclared block 'Ghost'"),
        (lambda o: o.pop(7), r"never inserted in main.py: \['create'\]"),
        (lambda o: o.insert(8, dict(o[7])), "inserted twice"),
        (lambda o: o.__setitem__(slice(6, 8), [o[7], o[6]]), "out of topology order"),
        (lambda o: o.pop(3), "before its CREATE_FILE"),
        (lambda o: o.__setitem__(slice(4, 6), [o[5], o[4]]), "follows group 1"),
        (lambda o: o[2].update(blank_before=3), "blank_before"),
        (lambda o: o[1].update(stmt="print('x')"), "not an import"),
        (lambda o: o[6].update(params={"app_name": 1}), "params"),
    ],
)
def test_a_malformed_operation_list_is_refused(mutate: Any, message: str) -> None:
    with pytest.raises(RecordError, match=message):
        validate_ops(_ops_with(mutate), TOPOLOGY)
