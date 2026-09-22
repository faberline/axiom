"""Build a non-deployable inventory from the reviewed Python seed fixtures.

The importer deliberately stops before formal split planning, corpus rows,
controller approval, or training.  It only makes the existing fixed fixtures
addressable by :mod:`som.python_corpus_materializer`.
"""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
from typing import Any

from .python_corpus import DOMAIN, FIXTURE_DIR, PROTOCOL, PythonCorpusError, recompute_fixed_oracle
from .python_oracle import _unsafe_python


class PythonSeedInventoryError(PythonCorpusError):
    """A seed fixture cannot enter the staged formal-material inventory."""


STATUS = "staged_not_formal"

# These artifacts are made by Python and test tools.  They are not declared
# fixture inputs, are never hashed into the inventory, and must not be copied
# into the fixed-fixture execution sandbox.  In particular, a cloud-backed
# stale ``__pycache__`` file must not be able to stall the oracle before it
# starts an owned test.
_GENERATED_CACHE_NAMES = frozenset({
    "__pycache__",
    ".pytest_cache",
    ".mypy_cache",
    ".ruff_cache",
    ".hypothesis",
    ".tox",
    ".nox",
    "htmlcov",
})
_GENERATED_CACHE_SUFFIXES = (".pyc", ".pyo")


def _ignore_generated_cache_files(_directory: str, names: list[str]) -> set[str]:
    """Return generated cache artifacts for ``shutil.copytree`` to skip.

    Fixture files remain explicit through ``family.json`` and their content
    hashes.  This filter only removes tool output; it never selects fixture
    inputs or changes the fixed oracle command.
    """
    return {
        name for name in names
        if name in _GENERATED_CACHE_NAMES or name.endswith(_GENERATED_CACHE_SUFFIXES)
    }


def _sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _dump(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def _fail(message: str) -> None:
    raise PythonSeedInventoryError(message)


def _owned_file(root: Path, relative: Any, label: str) -> Path:
    if not isinstance(relative, str) or not relative or Path(relative).is_absolute():
        _fail(f"{label} must be a non-empty relative path")
    raw = Path(relative)
    if any(part in {"", ".", ".."} for part in raw.parts):
        _fail(f"{label} is not a simple relative path: {relative}")
    path = root / raw
    if path.is_symlink() or not path.is_file():
        _fail(f"{label} is not an owned regular file: {relative}")
    try:
        path.resolve().relative_to(root.resolve())
    except ValueError as exc:
        raise PythonSeedInventoryError(f"{label} escapes the fixture root") from exc
    return path


def _family_paths(root: Path) -> list[Path]:
    base = root / FIXTURE_DIR
    if not base.is_dir():
        _fail(f"seed fixture directory is missing: {base}")
    paths = sorted(base.glob("**/family.json"))
    if not paths:
        _fail("seed fixture directory has no family.json files")
    return paths


def _load_family(root: Path, path: Path) -> tuple[dict[str, Any], dict[str, Any], dict[str, Any]]:
    try:
        raw = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise PythonSeedInventoryError(f"invalid seed family record: {path}") from exc
    if not isinstance(raw, dict) or raw.get("domain") not in {None, DOMAIN}:
        _fail(f"seed family has invalid domain: {path}")
    family = raw.get("family_id", raw.get("id"))
    lineage = raw.get("semantic_lineage")
    capability = raw.get("capability")
    requirement = raw.get("requirement")
    skeleton = raw.get("skeleton")
    if not all(isinstance(value, str) and value for value in (family, lineage, capability, requirement, skeleton)):
        _fail(f"seed family lacks ID, lineage, capability, requirement, or skeleton: {path}")
    if raw.get("status") not in {"draft", STATUS}:
        _fail(f"seed family has unsupported status: {path}")
    source_hashes = raw.get("source_hashes")
    test_hashes = raw.get("test_hashes", {})
    candidates = raw.get("candidates")
    if not isinstance(source_hashes, dict) or not isinstance(candidates, list) or len(candidates) != 7:
        _fail(f"seed family needs source hashes and exactly seven candidates: {path}")
    ids: set[str] = set()
    source_digests: set[str] = set()
    normalized: list[dict[str, Any]] = []
    oracle_records: dict[str, Any] = {}
    for number, candidate in enumerate(candidates):
        if not isinstance(candidate, dict):
            _fail(f"seed candidate is malformed: {path}")
        candidate_id = candidate.get("id")
        module = candidate.get("module")
        test = candidate.get("test")
        support_paths = candidate.get("support_paths", [])
        if not isinstance(candidate_id, str) or not candidate_id or candidate_id in ids:
            _fail(f"seed family repeats candidate ID: {path}")
        if not isinstance(module, str) or not isinstance(test, str) or not isinstance(support_paths, list):
            _fail(f"seed candidate paths are malformed: {path}/{candidate_id}")
        ids.add(candidate_id)
        family_root = path.parent
        family_relative = family_root.relative_to(root)
        def resolve_declared(relative: str, label: str) -> tuple[Path, str, str]:
            # Older reviewed seed records spell support paths from the corpus
            # root, while module/test paths are family-relative.  Normalize
            # both forms to an owned corpus-root path and a local hash key.
            declared = Path(relative)
            if declared.parts and declared.parts[0] == FIXTURE_DIR:
                fixed = _owned_file(root, relative, label)
                try:
                    hash_key = str(declared.relative_to(family_relative).as_posix())
                except ValueError as exc:
                    raise PythonSeedInventoryError(f"{label} names another family: {relative}") from exc
                root_relative = relative
            else:
                fixed = _owned_file(family_root, relative, label)
                hash_key = relative
                root_relative = str((family_relative / declared).as_posix())
            return fixed, hash_key, root_relative
        module_path, module_hash_key, module_root_relative = resolve_declared(module, f"{family}/{candidate_id} source")
        test_path, test_hash_key, test_root_relative = resolve_declared(test, f"{family}/{candidate_id} test")
        resolved_support = [resolve_declared(item, f"{family}/{candidate_id} support") for item in support_paths]
        all_root_relative = [module_root_relative, test_root_relative, *[item[2] for item in resolved_support]]
        if len(set(all_root_relative)) != len(all_root_relative):
            _fail(f"seed candidate repeats a source, test, or support path: {family}/{candidate_id}")
        hashes: dict[str, str] = {}
        declared_items = [(module_hash_key, module_root_relative, module_path), (test_hash_key, test_root_relative, test_path), *[(item[1], item[2], item[0]) for item in resolved_support]]
        for item_number, (hash_key, root_relative, fixed) in enumerate(declared_items):
            # Two reviewed seed formats exist: per-file maps and older
            # candidate-ID source/test maps.  Both bind exact bytes.
            if item_number == 0:
                expected = source_hashes.get(hash_key, source_hashes.get(root_relative, source_hashes.get(candidate_id)))
            elif item_number == 1:
                expected = source_hashes.get(hash_key, test_hashes.get(hash_key, test_hashes.get(root_relative, test_hashes.get(candidate_id))))
            else:
                expected = source_hashes.get(hash_key, candidate.get("support_sha256", {}).get(root_relative))
            actual = _sha(fixed.read_bytes())
            if not isinstance(expected, str) or expected != actual:
                _fail(f"seed source hash changed: {family}/{hash_key}")
            hashes[root_relative] = actual
            unsafe = _unsafe_python(fixed)
            if unsafe is not None:
                _fail(f"seed fixture is unsafe: {family}/{root_relative}: {unsafe}")
        source_digest = hashes[module_root_relative]
        if source_digest in source_digests:
            _fail(f"seed family repeats candidate source bytes: {family}")
        source_digests.add(source_digest)
        kind = "gold" if candidate_id in {"gold", "candidate_gold"} else "near_miss"
        root_relative = module_root_relative
        test_relative = test_root_relative
        support_relative = [item[2] for item in resolved_support]
        normalized.append({
            "id": candidate_id, "kind": kind, "source_path": root_relative,
            "source_sha256": source_digest, "tests": [test_relative],
            "test_sha256": {test_relative: hashes[test_relative]}, "support_files": support_relative,
            "support_sha256": {relative: hashes[relative] for relative in support_relative},
        })
        fixture_id = f"{family}:{candidate_id}"
        oracle_records[fixture_id] = {
            "source_path": root_relative, "source_sha256": source_digest,
            "tests": [{"path": test_relative, "sha256": hashes[test_relative]}],
            "support_files": [{"path": relative, "sha256": hashes[relative]} for relative in support_relative],
            "expected_exit_status": 0 if kind == "gold" else 1,
            "expected_pass": kind == "gold", "fixed_fixture_only": True,
        }
    if [item["kind"] for item in normalized].count("gold") != 1 or [item["kind"] for item in normalized].count("near_miss") != 6:
        _fail(f"seed family needs one gold and six near misses: {family}")
    inventory = {
        "family": family, "lineage": lineage, "capability": capability,
        "source_policy": raw.get("source_policy", "unknown"),
        "state": skeleton,
        "question": f"Which patch best satisfies this requirement? {requirement}",
        "prompt_provenance": "derived_from_fixed_seed_requirement_and_skeleton",
        "candidates": normalized,
    }
    ledger = {
        "family": family, "lineage": lineage, "capability": capability,
        "source_policy": raw.get("source_policy", "unknown"),
        "family_json": str(path.relative_to(root).as_posix()),
        "family_json_sha256": _sha(path.read_bytes()),
        "candidate_count": len(normalized), "status": STATUS,
    }
    return inventory, ledger, oracle_records


def stage_python_seed_inventory(*, fixture_root: Path, output_root: Path, run_oracle: bool = True) -> dict[str, Any]:
    """Import every current seed fixture into a separate non-formal inventory.

    The output is refused when non-empty.  Oracle evidence is written only
    after every owned candidate has matched its declared fixed-test outcome.
    """
    fixture_root = Path(fixture_root).resolve()
    output_root = Path(output_root).resolve()
    if output_root.exists() and any(output_root.iterdir()):
        _fail(f"refusing to overwrite staged inventory: {output_root}")
    if output_root == fixture_root or output_root.is_relative_to(fixture_root) or fixture_root.is_relative_to(output_root):
        _fail("staged inventory output must not overlap seed fixtures")
    families: list[dict[str, Any]] = []
    ledger: list[dict[str, Any]] = []
    oracle_records: dict[str, Any] = {}
    seen_families: set[str] = set()
    seen_lineages: set[str] = set()
    for path in _family_paths(fixture_root):
        inventory, entry, records = _load_family(fixture_root, path)
        if inventory["family"] in seen_families or inventory["lineage"] in seen_lineages:
            _fail(f"seed family repeats ID or semantic lineage: {path}")
        seen_families.add(inventory["family"]); seen_lineages.add(inventory["lineage"])
        families.append(inventory); ledger.append(entry); oracle_records.update(records)
    families.sort(key=lambda item: item["family"])
    ledger.sort(key=lambda item: item["family"])
    expected = len(families) * 7
    if len(oracle_records) != expected:
        _fail("seed candidate inventory coverage changed")
    output_root.mkdir(parents=True, exist_ok=True)
    inventory_doc = {
        "protocol": PROTOCOL, "domain": DOMAIN, "corpus_kind": "formal_material_inventory",
        "status": STATUS, "families": families,
        "source_root": str(fixture_root), "formal_split_plan": None,
        "training_authorized": False,
    }
    _dump(output_root / "inventory.json", inventory_doc)
    _dump(output_root / "family-ledger.json", {"protocol": PROTOCOL, "domain": DOMAIN, "status": STATUS, "families": ledger})
    report = {
        "protocol": PROTOCOL, "domain": DOMAIN, "status": STATUS,
        "coverage": {"families": len(families), "candidates": expected, "gold": len(families), "near_miss": expected - len(families)},
        "fixed_fixture_only": True, "executes_user_code": False,
        "oracle_executed": False, "records": {},
    }
    if run_oracle:
        observations = recompute_fixed_oracle(
            fixture_root,
            oracle_records,
            copy_ignore=_ignore_generated_cache_files,
        )
        # recompute_fixed_oracle itself rejects the first mismatch.  Check full
        # coverage before emitting any evidence file.
        if set(observations) != set(oracle_records):
            _fail("fixed oracle did not cover every staged seed candidate")
        report["oracle_executed"] = True
        report["records"] = {key: observations[key] for key in sorted(observations)}
    _dump(output_root / "reports" / "fixed-oracle-evidence.json", report)
    _dump(output_root / "manifest.json", {
        "protocol": PROTOCOL, "domain": DOMAIN, "corpus_kind": "formal_material_inventory", "status": STATUS,
        "inventory_sha256": _sha((output_root / "inventory.json").read_bytes()),
        "ledger_sha256": _sha((output_root / "family-ledger.json").read_bytes()),
        "oracle_evidence_sha256": _sha((output_root / "reports" / "fixed-oracle-evidence.json").read_bytes()),
        "coverage": report["coverage"], "formal_split_plan": None, "training_authorized": False,
    })
    return {"status": STATUS, "families": len(families), "candidates": expected,
            "oracle_executed": run_oracle, "output_root": str(output_root)}
