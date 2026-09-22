"""Offline materializer for the proposed formal Python SOM corpus.

This is deliberately a *data* boundary.  It copies only reviewed fixture
bytes named by an inventory and a frozen split plan.  It does not import,
compile, or execute fixture text.  The controller must run the fixed oracle
and approve the final packet in later, separate steps.
"""
from __future__ import annotations

import hashlib
import json
import shutil
from collections import Counter
from pathlib import Path
from typing import Any

from .python_corpus import DOMAIN, FAMILY_COUNTS, FIXTURE_DIR, PRESENCE, PROTOCOL, ROW_COUNTS, SPLITS, PythonCorpusError


class PythonCorpusMaterializeError(PythonCorpusError):
    """An owned fixture inventory cannot safely make a formal corpus."""


def _fail(message: str) -> None:
    raise PythonCorpusMaterializeError(message)


def _sha_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def _dump(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def _read_json(path: Path, label: str) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        _fail(f"{label} is missing: {path}")
    except json.JSONDecodeError as exc:
        _fail(f"{label} is invalid JSON: {exc}")
    if not isinstance(value, dict):
        _fail(f"{label} must be an object")
    return value


def _owned_file(root: Path, value: Any, label: str) -> Path:
    if not isinstance(value, str) or not value or Path(value).is_absolute():
        _fail(f"{label} must be a non-empty relative path")
    raw = Path(value)
    if any(part in {"", ".", ".."} for part in raw.parts):
        _fail(f"{label} is not a simple relative path: {value}")
    current = root.resolve()
    for part in raw.parts:
        current /= part
        if current.is_symlink():
            _fail(f"{label} contains a symlink: {value}")
    if not current.is_file() or not current.resolve().is_relative_to(root.resolve()):
        _fail(f"{label} is not an owned regular file: {value}")
    return current


def _candidate_files(root: Path, candidate: dict[str, Any], label: str) -> tuple[Path, list[Path], list[Path]]:
    source = _owned_file(root, candidate.get("source_path"), f"{label} source_path")
    raw_tests = candidate.get("tests")
    if not isinstance(raw_tests, list) or not raw_tests:
        _fail(f"{label} must name at least one fixed test")
    tests = [_owned_file(root, item, f"{label} test") for item in raw_tests]
    raw_support = candidate.get("support_files", [])
    if not isinstance(raw_support, list):
        _fail(f"{label} support_files must be a list")
    support = [_owned_file(root, item, f"{label} support") for item in raw_support]
    paths = [source, *tests, *support]
    if len({str(path) for path in paths}) != len(paths):
        _fail(f"{label} repeats a fixture path")
    return source, tests, support


def _load_inventory(root: Path, inventory_path: Path) -> dict[str, dict[str, Any]]:
    inventory = _read_json(inventory_path, "Python fixture inventory")
    if inventory.get("protocol") != PROTOCOL or inventory.get("domain") != DOMAIN:
        _fail("Python fixture inventory protocol changed")
    items = inventory.get("families")
    if not isinstance(items, list):
        _fail("Python fixture inventory families must be a list")
    result: dict[str, dict[str, Any]] = {}
    lineages: set[str] = set()
    for item in items:
        if not isinstance(item, dict):
            _fail("Python fixture inventory family is malformed")
        family, lineage = item.get("family"), item.get("lineage")
        if not isinstance(family, str) or not family or not isinstance(lineage, str) or not lineage:
            _fail("Python fixture inventory family or lineage is missing")
        if family in result or lineage in lineages:
            _fail("Python fixture inventory repeats a family or semantic lineage")
        if not isinstance(item.get("capability"), str) or not item["capability"]:
            _fail(f"Python inventory family {family} has no capability")
        if not isinstance(item.get("state"), str) or not isinstance(item.get("question"), str):
            _fail(f"Python inventory family {family} has no state or question")
        candidates = item.get("candidates")
        if not isinstance(candidates, list) or len(candidates) != 7:
            _fail(f"Python inventory family {family} needs exactly gold plus six near misses")
        candidate_ids: set[str] = set()
        source_hashes: set[str] = set()
        gold = 0
        normalized: list[dict[str, Any]] = []
        for candidate in candidates:
            if not isinstance(candidate, dict):
                _fail(f"Python inventory family {family} has malformed candidate")
            candidate_id = candidate.get("id")
            if not isinstance(candidate_id, str) or not candidate_id or candidate_id in candidate_ids:
                _fail(f"Python inventory family {family} has duplicate candidate source ID")
            candidate_ids.add(candidate_id)
            if candidate.get("kind") not in {"gold", "near_miss"}:
                _fail(f"Python inventory family {family} candidate {candidate_id} has invalid kind")
            gold += candidate["kind"] == "gold"
            source, tests, support = _candidate_files(root, candidate, f"Python inventory {family}/{candidate_id}")
            source_bytes = source.read_bytes()
            digest = _sha_bytes(source_bytes)
            if digest in source_hashes:
                _fail(f"Python inventory family {family} does not have seven distinct candidate sources")
            source_hashes.add(digest)
            normalized.append({**candidate, "_source": source, "_tests": tests, "_support": support, "_source_bytes": source_bytes})
        if gold != 1 or sum(item["kind"] == "near_miss" for item in normalized) != 6:
            _fail(f"Python inventory family {family} needs one gold and six near misses")
        result[family] = {**item, "candidates": normalized}
        lineages.add(lineage)
    return result


def _load_plan(path: Path, inventory: dict[str, dict[str, Any]]) -> dict[str, list[str]]:
    plan = _read_json(path, "Python frozen split plan")
    if plan.get("protocol") != PROTOCOL or plan.get("domain") != DOMAIN:
        _fail("Python frozen split plan protocol changed")
    raw_splits = plan.get("splits")
    if not isinstance(raw_splits, dict) or set(raw_splits) != set(SPLITS):
        _fail("Python frozen split plan must contain exactly train, validation, calibration, final")
    result: dict[str, list[str]] = {}
    seen: set[str] = set()
    lineages: set[str] = set()
    for split in SPLITS:
        families = raw_splits[split]
        if not isinstance(families, list) or len(families) != FAMILY_COUNTS[split]:
            _fail(f"Python frozen split plan {split} must contain {FAMILY_COUNTS[split]} families")
        if any(not isinstance(family, str) for family in families) or len(set(families)) != len(families):
            _fail(f"Python frozen split plan {split} repeats a family")
        for family in families:
            if family not in inventory:
                _fail(f"Python frozen split plan references unavailable family: {family}")
            if family in seen:
                _fail(f"Python family leaks across split plan: {family}")
            lineage = str(inventory[family]["lineage"])
            if lineage in lineages:
                _fail(f"Python semantic lineage leaks across split plan: {lineage}")
            seen.add(family); lineages.add(lineage)
        result[split] = sorted(families)
    return result


def _row_states(split: str, families: list[str]) -> list[tuple[str, bool]]:
    """Return stable (family, present) rows matching the frozen quotas."""
    total, present = ROW_COUNTS[split], PRESENCE[split][0]
    count, remainder = divmod(total, len(families))
    if remainder:
        _fail(f"internal frozen quota error for {split}")
    # Start each family at the floor proportion, then place the exact remainder
    # in deterministic family order.  This never uses candidate order or text.
    base, extra = divmod(present, len(families))
    states: list[tuple[str, bool]] = []
    for index, family in enumerate(families):
        present_for_family = base + (index < extra)
        if present_for_family > count:
            _fail(f"internal frozen presence quota error for {split}")
        states.extend((family, True) for _ in range(present_for_family))
        states.extend((family, False) for _ in range(count - present_for_family))
    if len(states) != total or sum(state for _, state in states) != present:
        _fail(f"internal materialized quota error for {split}")
    return states


def _copy_owned(source: Path, inventory_root: Path, output_root: Path, destination_relative: Path) -> tuple[str, str, bytes]:
    if not source.resolve().is_relative_to(inventory_root.resolve()):
        _fail(f"inventory file escapes root: {source}")
    destination = output_root / destination_relative
    destination.parent.mkdir(parents=True, exist_ok=True)
    data = source.read_bytes()
    destination.write_bytes(data)
    return str(destination_relative), _sha_bytes(data), data


def materialize_formal_python_corpus(
    *, inventory_root: Path, inventory_path: Path, split_plan_path: Path, output_root: Path
) -> dict[str, Any]:
    """Create a pending-review formal corpus from fixed owned fixture bytes.

    The output directory must be absent or empty.  This prevents a data build
    from replacing a reviewed or rejected corpus.  The returned corpus is not
    approved and cannot pass ``audit_formal_python_corpus`` until a separate
    oracle and controller-review phase supply their evidence.
    """
    inventory_root = Path(inventory_root).resolve()
    inventory_path = Path(inventory_path).resolve()
    split_plan_path = Path(split_plan_path).resolve()
    output_root = Path(output_root).resolve()
    if not inventory_root.is_dir():
        _fail(f"Python inventory root is missing: {inventory_root}")
    if not inventory_path.is_relative_to(inventory_root) or not split_plan_path.is_relative_to(inventory_root):
        _fail("Python inventory and frozen split plan must be owned by the inventory root")
    if output_root.exists() and any(output_root.iterdir()):
        _fail(f"refusing to overwrite non-empty formal corpus output: {output_root}")
    if output_root == inventory_root or output_root.is_relative_to(inventory_root) or inventory_root.is_relative_to(output_root):
        _fail("formal corpus output must not overlap the fixture inventory")
    inventory = _load_inventory(inventory_root, inventory_path)
    plan = _load_plan(split_plan_path, inventory)
    output_root.mkdir(parents=True, exist_ok=True)

    rows_by_split: dict[str, list[dict[str, Any]]] = {split: [] for split in SPLITS}
    ledger: list[dict[str, Any]] = []
    locked_files: dict[str, dict[str, str]] = {}
    oracle_plan: dict[str, dict[str, Any]] = {}
    for split in SPLITS:
        family_rows = Counter(family for family, _ in _row_states(split, plan[split]))
        for family_name in plan[split]:
            family = inventory[family_name]
            provenance = []
            for item in family["candidates"]:
                provenance.append({"id": item["id"], "sha256": _sha_bytes(item["_source_bytes"])})
            ledger.append({
                "family": family_name, "lineage": family["lineage"], "split": split,
                "capability": family["capability"], "source_policy": family.get("source_policy", "owned"),
                "candidate_provenance": provenance, "planned_rows": family_rows[family_name],
            })
        per_family_index: Counter[str] = Counter()
        for family_name, present in _row_states(split, plan[split]):
            family = inventory[family_name]
            row_index = per_family_index[family_name]
            per_family_index[family_name] += 1
            row_id = f"{family_name}:{split}:{row_index:02d}:{'present' if present else 'missing'}"
            gold = next(item for item in family["candidates"] if item["kind"] == "gold")
            near = [item for item in family["candidates"] if item["kind"] == "near_miss"]
            # Present and missing rows both have six candidates.  Omission is
            # stable and does not depend on input text or candidate placement.
            selected = near if not present else [gold] + [item for number, item in enumerate(near) if number != row_index % len(near)]
            candidates: list[dict[str, Any]] = []
            for item in selected:
                fixture_id = f"{row_id}:{item['id']}"
                base = Path(FIXTURE_DIR) / split / family_name / f"row-{row_index:02d}" / item["id"]
                source_rel, source_hash, source_bytes = _copy_owned(item["_source"], inventory_root, output_root, base / "candidate.py")
                locked_files[source_rel] = {"sha256": source_hash}
                tests: list[dict[str, str]] = []
                supports: list[dict[str, str]] = []
                for number, test in enumerate(item["_tests"]):
                    rel, digest, _ = _copy_owned(test, inventory_root, output_root, base / f"test-{number}.py")
                    locked_files[rel] = {"sha256": digest}; tests.append({"path": rel, "sha256": digest})
                for number, support in enumerate(item["_support"]):
                    rel, digest, _ = _copy_owned(support, inventory_root, output_root, base / f"support-{number}.py")
                    locked_files[rel] = {"sha256": digest}; supports.append({"path": rel, "sha256": digest})
                candidate = {
                    "id": fixture_id, "fixture_id": fixture_id, "provenance": item["id"],
                    "source_path": source_rel, "source_sha256": source_hash,
                    "support_files": supports, "tests": tests,
                    "text": source_bytes.decode("utf-8"), "oracle_pass": item["kind"] == "gold",
                }
                candidates.append(candidate)
                oracle_plan[fixture_id] = {
                    "source_path": source_rel, "source_sha256": source_hash, "support_files": supports,
                    "tests": tests, "expected_exit_status": 0 if item["kind"] == "gold" else 1,
                    "expected_pass": item["kind"] == "gold", "fixed_fixture_only": True,
                }
            rows_by_split[split].append({
                "id": row_id, "family": family_name, "lineage": family["lineage"], "capability": family["capability"],
                "state": family["state"], "question": family["question"], "candidates": candidates,
                "gold_candidate_id": f"{row_id}:{gold['id']}" if present else None,
                "missing_correct_patch": not present,
            })

    for split, rows in rows_by_split.items():
        if len(rows) != ROW_COUNTS[split] or sum(not row["missing_correct_patch"] for row in rows) != PRESENCE[split][0]:
            _fail(f"materialized {split} quota changed")
        if any(len(row["candidates"]) != 6 for row in rows):
            _fail(f"materialized {split} candidate count changed")
        (output_root / f"{split}-{DOMAIN}.jsonl").write_text(
            "".join(json.dumps(row, sort_keys=True) + "\n" for row in rows), encoding="utf-8"
        )
    _dump(output_root / "sources.lock.json", {"protocol": PROTOCOL, "domain": DOMAIN, "corpus_kind": "formal", "files": dict(sorted(locked_files.items()))})
    _dump(output_root / "family-ledger.json", ledger)
    _dump(output_root / "reports" / "oracle-plan.json", {"protocol": PROTOCOL, "domain": DOMAIN, "corpus_kind": "formal", "executes_user_code": False, "candidates": oracle_plan})
    _dump(output_root / "reports" / "final-review-packet.json", {
        "protocol": PROTOCOL, "domain": DOMAIN, "corpus_kind": "formal", "status": "pending_controller_review",
        "entries": [{"family": item["family"], "status": "pending", "reviewer_name": None, "approval_sha256": None} for item in ledger if item["split"] == "final"],
    })
    split_files = {f"{split}-{DOMAIN}.jsonl": _sha_bytes((output_root / f"{split}-{DOMAIN}.jsonl").read_bytes()) for split in SPLITS}
    manifest = {
        "protocol": PROTOCOL, "domain": DOMAIN, "corpus_kind": "formal", "corpus_status": "pending_controller_review",
        "fixture_root": FIXTURE_DIR, "files": split_files,
        "sources_lock_sha256": _sha_bytes((output_root / "sources.lock.json").read_bytes()),
        "family_ledger_sha256": _sha_bytes((output_root / "family-ledger.json").read_bytes()),
        "final_review_packet_sha256": _sha_bytes((output_root / "reports" / "final-review-packet.json").read_bytes()),
        "inventory_sha256": _sha_bytes(inventory_path.read_bytes()), "split_plan_sha256": _sha_bytes(split_plan_path.read_bytes()),
        "next_required_step": "run fixed oracle, then controller-review all final families",
    }
    _dump(output_root / "manifest.json", manifest)
    return {"status": "pending_controller_review", "rows": {split: len(rows) for split, rows in rows_by_split.items()},
            "families": {split: len(plan[split]) for split in SPLITS}, "candidates": len(oracle_plan),
            "output_root": str(output_root), "manifest_sha256": _sha_bytes((output_root / "manifest.json").read_bytes())}
