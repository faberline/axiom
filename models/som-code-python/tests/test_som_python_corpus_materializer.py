"""CPU-only checks for the offline formal Python corpus materializer."""
from __future__ import annotations

import hashlib
import json
from pathlib import Path

import pytest

from som.python_corpus_materializer import PythonCorpusMaterializeError, materialize_formal_python_corpus


COUNTS = {"train": 100, "validation": 40, "calibration": 40, "final": 60}


def _write(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def _inventory(root: Path, *, family_count: int = 240, duplicate_candidate_source: bool = False) -> tuple[Path, Path]:
    families = []
    names = []
    for number in range(family_count):
        name = f"family-{number:03d}"
        names.append(name)
        candidates = []
        for candidate_number in range(7):
            source_name = "candidate-0.py" if duplicate_candidate_source and candidate_number == 1 else f"candidate-{candidate_number}.py"
            source = f"fixtures/{name}/{source_name}"
            test = f"fixtures/{name}/test-{candidate_number}.py"
            _write(root / source, f"VALUE = {number * 10 + candidate_number}\n")
            _write(root / test, "def test_fixed_fixture():\n    assert True\n")
            candidates.append({"id": "gold" if candidate_number == 0 else f"miss-{candidate_number}",
                               "kind": "gold" if candidate_number == 0 else "near_miss",
                               "source_path": source, "tests": [test], "support_files": []})
        families.append({"family": name, "lineage": f"lineage-{number:03d}", "capability": "fastapi.route",
                         "state": f"fixed state {number}", "question": f"fixed question {number}", "candidates": candidates})
    inventory_path = root / "inventory.json"
    _write(inventory_path, json.dumps({"protocol": "som-v1", "domain": "python", "families": families}, sort_keys=True))
    plan = {}
    start = 0
    for split, amount in COUNTS.items():
        plan[split] = names[start:start + amount]
        start += amount
    plan_path = root / "split-plan.json"
    _write(plan_path, json.dumps({"protocol": "som-v1", "domain": "python", "splits": plan}, sort_keys=True))
    return inventory_path, plan_path


def _rows(root: Path, split: str) -> list[dict]:
    return [json.loads(line) for line in (root / f"{split}-python.jsonl").read_text(encoding="utf-8").splitlines()]


def test_materializes_deterministically_with_frozen_quotas(tmp_path: Path) -> None:
    inventory, plan = _inventory(tmp_path / "inventory")
    first = materialize_formal_python_corpus(inventory_root=inventory.parent, inventory_path=inventory,
                                             split_plan_path=plan, output_root=tmp_path / "one")
    second = materialize_formal_python_corpus(inventory_root=inventory.parent, inventory_path=inventory,
                                              split_plan_path=plan, output_root=tmp_path / "two")
    assert first["rows"] == {"train": 1200, "validation": 160, "calibration": 240, "final": 180}
    assert first["families"] == COUNTS
    assert first["candidates"] == 10_680
    assert second["manifest_sha256"] == first["manifest_sha256"]
    for split, expected in (("train", (1200, 600)), ("validation", (160, 107)), ("calibration", (240, 160)), ("final", (180, 120))):
        rows = _rows(tmp_path / "one", split)
        assert len(rows) == expected[0]
        assert sum(not row["missing_correct_patch"] for row in rows) == expected[1]
        assert all(len(row["candidates"]) == 6 for row in rows)
        assert (tmp_path / "one" / f"{split}-python.jsonl").read_bytes() == (tmp_path / "two" / f"{split}-python.jsonl").read_bytes()


def test_materialized_candidate_text_is_exact_owned_fixture_bytes(tmp_path: Path) -> None:
    inventory, plan = _inventory(tmp_path / "inventory")
    output = tmp_path / "out"
    materialize_formal_python_corpus(inventory_root=inventory.parent, inventory_path=inventory, split_plan_path=plan, output_root=output)
    row = _rows(output, "final")[0]
    candidate = row["candidates"][0]
    source = output / candidate["source_path"]
    assert candidate["text"] == source.read_text(encoding="utf-8")
    assert candidate["source_sha256"] == hashlib.sha256(source.read_bytes()).hexdigest()
    lock = json.loads((output / "sources.lock.json").read_text(encoding="utf-8"))
    assert lock["files"][candidate["source_path"]]["sha256"] == candidate["source_sha256"]


def test_refuses_insufficient_family_inventory(tmp_path: Path) -> None:
    inventory, plan = _inventory(tmp_path / "inventory", family_count=239)
    with pytest.raises(PythonCorpusMaterializeError, match="final must contain 60 families"):
        materialize_formal_python_corpus(inventory_root=inventory.parent, inventory_path=inventory,
                                         split_plan_path=plan, output_root=tmp_path / "out")


def test_refuses_family_leakage_in_frozen_plan(tmp_path: Path) -> None:
    inventory, plan = _inventory(tmp_path / "inventory")
    value = json.loads(plan.read_text(encoding="utf-8"))
    value["splits"]["validation"][0] = value["splits"]["train"][0]
    _write(plan, json.dumps(value, sort_keys=True))
    with pytest.raises(PythonCorpusMaterializeError, match="leaks across split plan"):
        materialize_formal_python_corpus(inventory_root=inventory.parent, inventory_path=inventory,
                                         split_plan_path=plan, output_root=tmp_path / "out")


def test_refuses_non_distinct_candidate_sources(tmp_path: Path) -> None:
    inventory, plan = _inventory(tmp_path / "inventory", duplicate_candidate_source=True)
    with pytest.raises(PythonCorpusMaterializeError, match="seven distinct candidate sources"):
        materialize_formal_python_corpus(inventory_root=inventory.parent, inventory_path=inventory,
                                         split_plan_path=plan, output_root=tmp_path / "out")
