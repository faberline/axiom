"""Import authored Python material packs into a staged, non-formal inventory.

This boundary is intentionally smaller than the formal corpus builder.  It
audits source and metadata only, and never imports or runs a candidate or a
test.  The returned inventory is useful for later review, but it cannot
authorize a split, oracle, or training run.
"""
from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any, Iterable

from .python_candidate_quality import audit_staged_candidate_quality
from .python_corpus import DOMAIN, PROTOCOL, PythonCorpusError
from .python_oracle import _unsafe_python

STATUS = "staged_not_formal"


class PythonMaterialInventoryError(PythonCorpusError):
    """A material pack is not safe to enter the staged inventory."""


def _sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _fail(message: str) -> None:
    raise PythonMaterialInventoryError(message)


def _owned(root: Path, value: Any, label: str) -> Path:
    if not isinstance(value, str) or not value or Path(value).is_absolute():
        _fail(f"{label} must be a relative path")
    raw = Path(value)
    if any(part in {"", ".", ".."} for part in raw.parts):
        _fail(f"{label} is not a simple relative path: {value}")
    path = root / raw
    if path.is_symlink() or not path.is_file() or not path.resolve().is_relative_to(root.resolve()):
        _fail(f"{label} is not an owned regular file: {value}")
    return path


def _family(pack: Path, metadata_path: Path, common_root: Path) -> dict[str, Any]:
    try:
        metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        _fail(f"invalid family metadata: {metadata_path}: {exc}")
    if not isinstance(metadata, dict):
        _fail(f"family metadata must be an object: {metadata_path}")
    family_id = metadata.get("family_id")
    lineage = metadata.get("semantic_lineage")
    capability = metadata.get("capability")
    requirement = metadata.get("requirement")
    skeleton = metadata.get("skeleton")
    if not all(isinstance(value, str) and value for value in (family_id, lineage, capability, requirement, skeleton)):
        _fail(f"family metadata lacks required fields: {metadata_path}")
    candidates = metadata.get("candidates")
    if not isinstance(candidates, list) or len(candidates) != 7:
        _fail(f"{family_id} must contain exactly seven candidates")
    ids: set[str] = set()
    normalized: list[dict[str, Any]] = []
    for entry in candidates:
        if not isinstance(entry, dict):
            _fail(f"{family_id} has malformed candidate metadata")
        cid, module, test = entry.get("id"), entry.get("module"), entry.get("test")
        if not isinstance(cid, str) or not cid or cid in ids:
            _fail(f"{family_id} repeats or lacks a candidate ID")
        ids.add(cid)
        family_root = metadata_path.parent
        source = _owned(family_root, module, f"{family_id}/{cid} module")
        test_path = _owned(family_root, test, f"{family_id}/{cid} test")
        support_paths = entry.get("support_paths", [])
        if not isinstance(support_paths, list):
            _fail(f"{family_id}/{cid} support_paths must be a list")
        supports = [_owned(family_root, item, f"{family_id}/{cid} support") for item in support_paths]
        files = [source, test_path, *supports]
        if len({str(path) for path in files}) != len(files):
            _fail(f"{family_id}/{cid} repeats a fixture path")
        # Candidate source must be statically safe.  Tests are fixed oracle
        # inputs; they may contain the controlled import mechanism used by
        # the later oracle, but this importer never executes them.
        unsafe = _unsafe_python(source)
        if unsafe is not None:
            _fail(f"unsafe candidate {source}: {unsafe}")
        source_rel = source.resolve().relative_to(common_root.resolve()).as_posix()
        test_rel = test_path.resolve().relative_to(common_root.resolve()).as_posix()
        support_rel = [path.resolve().relative_to(common_root.resolve()).as_posix() for path in supports]
        normalized.append({
            "id": cid,
            "kind": "gold" if cid == "gold" else "near_miss",
            "source_path": source_rel,
            "source_sha256": _sha(source),
            "tests": [test_rel],
            "test_sha256": {test_rel: _sha(test_path)},
            "support_files": support_rel,
            "support_sha256": {path: _sha(item) for path, item in zip(support_rel, supports)},
            **({"failure_mode": entry["failure_mode"]} if "failure_mode" in entry else {}),
        })
    if sum(item["kind"] == "gold" for item in normalized) != 1 or sum(item["kind"] == "near_miss" for item in normalized) != 6:
        _fail(f"{family_id} must have one gold and six near misses")
    family_rel = metadata_path.resolve().relative_to(common_root.resolve()).as_posix()
    return {
        "family": family_id,
        "lineage": lineage,
        "capability": capability,
        "source_policy": metadata.get("source_policy", "authored"),
        "state": skeleton,
        "question": f"Which patch best satisfies this requirement? {requirement}",
        "prompt_provenance": "derived_from_staged_family_metadata",
        "candidates": normalized,
        "material_family_json": family_rel,
        "material_family_json_sha256": _sha(metadata_path),
    }


def build_staged_material_inventory(pack_roots: Iterable[str | Path], *, output_root: str | Path | None = None) -> dict[str, Any]:
    """Audit and translate packs, returning ``staged_not_formal`` inventory.

    ``output_root`` is optional and may contain only generated staged reports.
    No formal rows, split plan, oracle evidence, or approvals are produced.
    """
    packs = [Path(item).resolve() for item in pack_roots]
    if not packs or any(not pack.is_dir() for pack in packs):
        _fail("all material pack roots must exist")
    # Both checked-in packs share ``data/som/python-v1`` as their owned root.
    common_root = packs[0].parents[1] if packs[0].name in {"core-python", "fastapi-pydantic"} else packs[0].parent
    reports = []
    families: list[dict[str, Any]] = []
    seen_family: set[str] = set(); seen_lineage: set[str] = set()
    for pack in packs:
        report = audit_staged_candidate_quality(pack)
        reports.append(report)
        if report["status"] != "passed":
            _fail(f"candidate quality audit failed for {pack}")
        for metadata_path in sorted(pack.rglob("family.json")):
            item = _family(pack, metadata_path, common_root)
            if item["family"] in seen_family or item["lineage"] in seen_lineage:
                _fail(f"duplicate family or lineage: {item['family']}")
            seen_family.add(item["family"]); seen_lineage.add(item["lineage"])
            families.append(item)
    families.sort(key=lambda item: item["family"])
    inventory = {"protocol": PROTOCOL, "domain": DOMAIN, "corpus_kind": "staged_material_inventory", "status": STATUS,
                 "families": families, "formal_split_plan": None, "training_authorized": False,
                 "fixed_fixture_only": True, "executes_user_code": False,
                 "quality_audits": reports}
    if output_root is not None:
        destination = Path(output_root).resolve()
        destination.mkdir(parents=True, exist_ok=True)
        (destination / "inventory.json").write_text(json.dumps(inventory, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return inventory
