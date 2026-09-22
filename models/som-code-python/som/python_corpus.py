"""Formal Python SOM corpus contract.

This module validates only repository-owned training records.  It never runs a
request, imports a candidate, or accepts a hand-written passing audit report.
The executable oracle has a separate, fixed-fixture runner in
:mod:`som.python_oracle`.

Required layout::

    data/som/python-v1/
      manifest.json
      sources.lock.json
      family-ledger.json
      {train,validation,calibration,final}-python.jsonl
      proposed-fixtures/<family>/<candidate-id>/candidate.py
      proposed-fixtures/<family>/<candidate-id>/test_candidate.py
      reports/oracle-evidence.json
      reports/final-review-packet.json

A smoke fixture set can use ``proposed-fixtures`` and the oracle format, but
it must declare ``corpus_kind: smoke``.  This formal validator requires
``corpus_kind: formal`` and the complete frozen split quotas.
"""
from __future__ import annotations

import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
from collections import Counter
from pathlib import Path
from typing import Any, Callable

from .paths import sha256
from .python_oracle import _unsafe_fixed_test, _unsafe_python

PROTOCOL = "som-v1"
DOMAIN = "python"
SPLITS = ("train", "validation", "calibration", "final")
ROW_COUNTS = {"train": 1200, "validation": 160, "calibration": 240, "final": 180}
PRESENCE = {"train": (600, 600), "validation": (107, 53), "calibration": (160, 80), "final": (120, 60)}
FAMILY_COUNTS = {"train": 100, "validation": 40, "calibration": 40, "final": 60}
FIXTURE_DIR = "proposed-fixtures"


class PythonCorpusError(ValueError):
    """The formal Python corpus does not meet its frozen contract."""


def _check(value: bool, message: str) -> None:
    if not value:
        raise PythonCorpusError(message)


def _json(path: Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise PythonCorpusError(f"required Python corpus file is missing: {path}") from exc
    except json.JSONDecodeError as exc:
        raise PythonCorpusError(f"invalid JSON in {path}: {exc}") from exc


def _jsonl(path: Path) -> list[dict[str, Any]]:
    try:
        values = [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line.strip()]
    except FileNotFoundError as exc:
        raise PythonCorpusError(f"required Python corpus split is missing: {path}") from exc
    except json.JSONDecodeError as exc:
        raise PythonCorpusError(f"invalid JSONL in {path}: {exc}") from exc
    _check(all(isinstance(value, dict) for value in values), f"split contains a non-object row: {path}")
    return values


def _relative_owned(root: Path, value: Any, label: str) -> Path:
    _check(isinstance(value, str) and value and not Path(value).is_absolute(), f"{label} must be a relative path")
    resolved = (root / value).resolve()
    try:
        resolved.relative_to(root.resolve())
    except ValueError as exc:
        raise PythonCorpusError(f"{label} escapes the Python corpus root") from exc
    _check(resolved.is_file(), f"{label} is missing: {value}")
    return resolved


def _sha_text(value: str) -> str:
    return hashlib.sha256(value.encode("utf-8")).hexdigest()


def _source_lock(root: Path, path: Path) -> dict[str, str]:
    lock = _json(path)
    _check(isinstance(lock, dict), "Python source lock must be an object")
    _check(lock.get("protocol") == PROTOCOL and lock.get("domain") == DOMAIN, "Python source lock protocol changed")
    files = lock.get("files")
    _check(isinstance(files, dict) and files, "Python source lock has no file hashes")
    normalized: dict[str, str] = {}
    for relative, entry in files.items():
        _check(isinstance(relative, str) and isinstance(entry, dict), "Python source lock entry is malformed")
        digest = entry.get("sha256")
        _check(isinstance(digest, str) and len(digest) == 64, "Python source lock hash is malformed")
        file_path = _relative_owned(root, relative, "Python source lock path")
        _check(sha256(file_path) == digest, f"Python source lock hash changed: {relative}")
        normalized[relative] = digest
    return normalized


def _validate_manifest(root: Path) -> dict[str, Any]:
    manifest_path = root / "manifest.json"
    manifest = _json(manifest_path)
    _check(isinstance(manifest, dict), "Python manifest must be an object")
    _check(manifest.get("protocol") == PROTOCOL and manifest.get("domain") == DOMAIN, "Python corpus protocol changed")
    _check(manifest.get("corpus_kind") == "formal", "Python smoke fixtures cannot masquerade as formal data")
    _check(manifest.get("corpus_status") == "ready_for_controller_review", "Python corpus is not ready for controller review")
    _check(manifest.get("fixture_root") == FIXTURE_DIR, "Python fixture layout changed")
    files = manifest.get("files")
    _check(isinstance(files, dict), "Python manifest has no split hashes")
    for split in SPLITS:
        name = f"{split}-{DOMAIN}.jsonl"
        _check(isinstance(files.get(name), str) and len(files[name]) == 64, f"Python manifest has no hash for {name}")
        _check(files[name] == sha256(root / name), f"Python split bytes changed: {name}")
    for key, relative in (
        ("sources_lock_sha256", "sources.lock.json"),
        ("family_ledger_sha256", "family-ledger.json"),
        ("final_review_packet_sha256", "reports/final-review-packet.json"),
    ):
        _check(isinstance(manifest.get(key), str) and len(manifest[key]) == 64, f"Python manifest has no {key}")
        _check(manifest[key] == sha256(root / relative), f"Python manifest binding changed: {relative}")
    return manifest


def _rows(root: Path) -> dict[str, list[dict[str, Any]]]:
    return {split: _jsonl(root / f"{split}-{DOMAIN}.jsonl") for split in SPLITS}


def _validate_rows(root: Path, loaded: dict[str, list[dict[str, Any]]], source_lock: dict[str, str]) -> dict[str, dict[str, dict[str, Any]]]:
    expected: dict[str, dict[str, dict[str, Any]]] = {}
    seen_fixture_ids: set[str] = set()
    for split, entries in loaded.items():
        _check(len(entries) == ROW_COUNTS[split], f"Python {split} row count changed")
        present, missing = PRESENCE[split]
        _check(sum(not bool(row.get("missing_correct_patch")) for row in entries) == present, f"Python {split} present quota changed")
        _check(sum(bool(row.get("missing_correct_patch")) for row in entries) == missing, f"Python {split} missing quota changed")
        expected[split] = {}
        for row in entries:
            family = row.get("family")
            _check(isinstance(family, str) and family, "Python row has no family")
            candidates = row.get("candidates")
            _check(isinstance(candidates, list) and 2 <= len(candidates) <= 7, "Python candidate count is outside 2-7")
            ids = [candidate.get("id") for candidate in candidates if isinstance(candidate, dict)]
            _check(len(ids) == len(candidates) and all(isinstance(item, str) and item for item in ids), "Python candidate ID is missing")
            _check(len(set(ids)) == len(ids), "Python row has duplicate candidate IDs")
            passes = []
            for candidate in candidates:
                fixture_id = candidate.get("fixture_id")
                _check(isinstance(fixture_id, str) and fixture_id, "Python candidate has no fixture_id")
                _check(fixture_id not in seen_fixture_ids, "Python fixture candidate is reused")
                seen_fixture_ids.add(fixture_id)
                source_path = candidate.get("source_path")
                source_hash = candidate.get("source_sha256")
                _check(isinstance(source_path, str) and source_path.startswith(FIXTURE_DIR + "/"), "Python candidate source is outside proposed-fixtures")
                _check(isinstance(source_hash, str) and len(source_hash) == 64, "Python candidate source hash is missing")
                _check(isinstance(candidate.get("text"), str), "Python candidate text is missing")
                source = _relative_owned(root, source_path, "Python candidate source")
                _check(sha256(source) == source_hash, "Python candidate source bytes changed")
                try:
                    source_text = source.read_bytes().decode("utf-8")
                except UnicodeDecodeError as exc:
                    raise PythonCorpusError("Python candidate source is not UTF-8") from exc
                _check(candidate["text"] == source_text, "Python candidate text differs from its executable source")
                support_files = _support_files(root, candidate, f"Python candidate {fixture_id}")
                for support in support_files:
                    _check(source_lock.get(support["path"]) == support["sha256"], "Python candidate support file is not locked")
                expected[split][fixture_id] = {"source_path": source_path, "source_sha256": source_hash,
                                               "support_files": support_files,
                                               "oracle_pass": bool(candidate.get("oracle_pass")), "text": candidate["text"], "family": family}
                if candidate.get("oracle_pass"):
                    passes.append(candidate)
            if row.get("missing_correct_patch"):
                _check(not passes and row.get("gold_candidate_id") is None, "Python missing row includes a passing patch")
            else:
                _check(len(passes) == 1 and row.get("gold_candidate_id") == passes[0]["id"], "Python gold oracle mismatch")
    return expected


def _validate_ledger(root: Path, loaded: dict[str, list[dict[str, Any]]]) -> list[dict[str, Any]]:
    ledger = _json(root / "family-ledger.json")
    _check(isinstance(ledger, list), "Python family ledger must be a list")
    _check(len(ledger) == sum(FAMILY_COUNTS.values()), "Python family ledger count changed")
    family_seen: set[str] = set()
    lineage_seen: set[str] = set()
    actual = Counter()
    row_families = {split: {str(row.get("family")) for row in entries} for split, entries in loaded.items()}
    for item in ledger:
        _check(isinstance(item, dict), "Python family ledger item is malformed")
        family, lineage, split = item.get("family"), item.get("lineage"), item.get("split")
        _check(isinstance(family, str) and family and isinstance(lineage, str) and lineage, "Python family or lineage is missing")
        _check(split in SPLITS, "Python family has invalid split")
        _check(family not in family_seen and lineage not in lineage_seen, "Python family lineage leaks across splits")
        _check(family in row_families[split], "Python ledger family has no rows in its split")
        provenance = item.get("candidate_provenance")
        _check(isinstance(provenance, list) and len(provenance) == 7, "Python family does not have gold plus six near misses")
        hashes = [entry.get("sha256") for entry in provenance if isinstance(entry, dict)]
        _check(len(hashes) == 7 and all(isinstance(value, str) and len(value) == 64 for value in hashes), "Python provenance hash is missing")
        _check(len(set(hashes)) == 7, "Python family has duplicate candidate provenance")
        family_seen.add(family); lineage_seen.add(lineage); actual[split] += 1
    _check(dict(actual) == FAMILY_COUNTS, "Python family split quota changed")
    for split, families in row_families.items():
        _check(families <= family_seen, f"Python {split} row refers to an unknown family")
    return ledger


def _canonical_command(test_paths: list[str]) -> list[str]:
    return ["python", "-m", "pytest", "-q", "--disable-warnings", *test_paths]


def _support_files(root: Path, record: dict[str, Any], label: str) -> list[dict[str, str]]:
    """Validate explicitly declared support modules and their exact bytes."""
    raw = record.get("support_files")
    if raw is None:
        paths = record.get("support_paths")
        hashes = record.get("support_sha256")
        _check(isinstance(paths, list) and isinstance(hashes, dict), f"{label} support files are missing")
        _check(set(hashes) == set(paths), f"{label} support paths and hashes differ")
        raw = [{"path": path, "sha256": hashes[path]} for path in paths]
    _check(isinstance(raw, list), f"{label} support files are missing")
    result: list[dict[str, str]] = []
    seen: set[str] = set()
    for item in raw:
        _check(isinstance(item, dict), f"{label} support file record is malformed")
        path_value, digest = item.get("path"), item.get("sha256")
        _check(isinstance(path_value, str) and path_value.startswith(FIXTURE_DIR + "/"),
               f"{label} support file is outside proposed-fixtures")
        _check(path_value not in seen, f"{label} support file is duplicated")
        _check(isinstance(digest, str) and len(digest) == 64, f"{label} support file hash is missing")
        path = _relative_owned(root, path_value, f"{label} support file")
        _check(path.suffix == ".py", f"{label} support file must be Python")
        _check(sha256(path) == digest, f"{label} support file bytes changed: {path_value}")
        reason = _unsafe_python(path)
        _check(reason is None, reason or f"{label} support file is unsafe")
        seen.add(path_value)
        result.append({"path": path_value, "sha256": digest})
    return result


def recompute_fixed_oracle(
    root: Path,
    records: dict[str, dict[str, Any]],
    *,
    copy_ignore: Callable[[str, list[str]], set[str]] | None = None,
    batch_size: int = 7,
) -> dict[str, dict[str, Any]]:
    """Re-run only the owned fixed fixtures and return fresh observations.

    ``records`` supplies only paths and frozen expected exits.  The command is
    fixed here.  This function has no request argument and refuses files that
    escape ``proposed-fixtures`` or contain the oracle runner's blocked forms.
    """
    root = Path(root).resolve()
    _check(isinstance(batch_size, int) and not isinstance(batch_size, bool) and batch_size > 0,
           "Python oracle batch size must be a positive integer")
    _check(isinstance(records, dict) and records, "Python oracle has no records to recompute")
    checked: list[tuple[str, Path, list[Path], list[dict[str, str]], int]] = []
    for fixture_id, record in records.items():
        _check(isinstance(fixture_id, str) and fixture_id and isinstance(record, dict), "Python oracle record is malformed")
        source = _relative_owned(root, record.get("source_path"), "Python oracle source")
        _check(str(record["source_path"]).startswith(FIXTURE_DIR + "/"), "Python oracle source is outside proposed-fixtures")
        expected_exit = record.get("expected_exit_status")
        _check(isinstance(expected_exit, int), "Python oracle expected exit status is missing")
        tests_data = record.get("tests")
        _check(isinstance(tests_data, list) and tests_data, "Python oracle tests are missing")
        tests: list[Path] = []
        for test in tests_data:
            _check(isinstance(test, dict), "Python oracle test record is malformed")
            test_path = test.get("path")
            _check(isinstance(test_path, str) and test_path.startswith(FIXTURE_DIR + "/"), "Python oracle test is outside proposed-fixtures")
            tests.append(_relative_owned(root, test_path, "Python oracle test"))
        support_files = _support_files(root, record, f"Python oracle {fixture_id}")
        supports = [_relative_owned(root, item["path"], "Python oracle support file") for item in support_files]
        for path in [source, *supports]:
            reason = _unsafe_python(path)
            _check(reason is None, reason or "Python oracle fixture is unsafe")
        for path in tests:
            reason = _unsafe_fixed_test(path)
            _check(reason is None, reason or "Python fixed test is unsafe")
        checked.append((fixture_id, source, tests, support_files, expected_exit))

    fresh: dict[str, dict[str, Any]] = {}
    # A full corpus can contain thousands of candidates.  Staging it once for
    # every candidate is needlessly slow, while staging it all at once can make
    # a small smoke corpus exceed its bounded execution window.  The records
    # were completely checked above, so batches only change the staging scope.
    for start in range(0, len(checked), batch_size):
        batch = checked[start:start + batch_size]
        if copy_ignore is None:
            fresh.update(_run_fixed_oracle_batch(root, batch))
        else:
            fresh.update(_run_fixed_oracle_batch(root, batch, copy_ignore=copy_ignore))
    return fresh


def _run_fixed_oracle_batch(
    root: Path,
    checked: list[tuple[str, Path, list[Path], list[dict[str, str]], int]],
    *,
    copy_ignore: Callable[[str, list[str]], set[str]] | None = None,
) -> dict[str, dict[str, Any]]:
    """Run a bounded, already-validated group of fixed oracle fixtures."""
    fresh: dict[str, dict[str, Any]] = {}
    with tempfile.TemporaryDirectory(prefix="som-v1-python-formal-") as temp_name:
        staged_root = Path(temp_name) / "python-v1"
        shutil.copytree(root, staged_root, ignore=copy_ignore)
        for fixture_id, source, tests, support_files, expected_exit in checked:
            staged_source = staged_root / source.relative_to(root)
            staged_tests = [staged_root / test.relative_to(root) for test in tests]
            relative_tests = [str(test.relative_to(root)) for test in tests]
            command = _canonical_command(relative_tests)
            work = staged_root / ".oracle-work" / fixture_id
            home, temp = work / "home", work / "tmp"
            home.mkdir(parents=True, exist_ok=True); temp.mkdir(parents=True, exist_ok=True)
            env = {"PATH": os.environ.get("PATH", ""), "HOME": str(home), "TMPDIR": str(temp),
                   "PYTHONHASHSEED": "0", "PYTHONDONTWRITEBYTECODE": "1", "PYTEST_DISABLE_PLUGIN_AUTOLOAD": "1",
                   "PYTHONPATH": os.pathsep.join((str(staged_source.parent), str(staged_root)))}
            proc = subprocess.run([sys.executable, "-m", "pytest", "-q", "--disable-warnings", *relative_tests],
                                  cwd=staged_root, env=env, text=True, capture_output=True, timeout=30, check=False)
            observed_pass = proc.returncode == 0
            _check((expected_exit == 0) == (proc.returncode == 0), "Python oracle expected exit does not match fresh result")
            _check(proc.returncode == expected_exit, "Python oracle fresh exit status changed")
            fresh[fixture_id] = {
                "source_path": str(source.relative_to(root)), "source_sha256": sha256(staged_source),
                "source_text": staged_source.read_bytes().decode("utf-8"),
                "tests": [{"path": str(test.relative_to(root)), "sha256": sha256(staged_root / test.relative_to(root))} for test in tests],
                "support_files": [{"path": item["path"], "sha256": sha256(staged_root / item["path"])} for item in support_files],
                "command": command, "expected_exit_status": expected_exit,
                "exit_status": proc.returncode, "observed_pass": observed_pass,
                "fixed_fixture_only": True,
            }
    return fresh


def _validate_evidence(root: Path, expected: dict[str, dict[str, dict[str, Any]]], source_lock: dict[str, str], *, oracle_batch_size: int = 7) -> dict[str, Any]:
    evidence_path = root / "reports" / "oracle-evidence.json"
    evidence = _json(evidence_path)
    _check(isinstance(evidence, dict), "Python oracle evidence must be an object")
    _check(evidence.get("protocol") == PROTOCOL and evidence.get("domain") == DOMAIN, "Python oracle evidence protocol changed")
    _check(evidence.get("executes_user_code") is False, "Python oracle evidence has an unsafe execution boundary")
    _check(evidence.get("fixture_root") == FIXTURE_DIR, "Python oracle evidence fixture root changed")
    records = evidence.get("candidates")
    _check(isinstance(records, dict), "Python oracle evidence has no candidate records")
    flat = {fixture_id: item for values in expected.values() for fixture_id, item in values.items()}
    _check(set(records) == set(flat), "Python oracle evidence does not cover every candidate")
    canonical_records: list[dict[str, Any]] = []
    for fixture_id, required in flat.items():
        record = records[fixture_id]
        _check(isinstance(record, dict), "Python oracle candidate record is malformed")
        _check(record.get("source_path") == required["source_path"], "Python oracle source path mismatch")
        _check(record.get("source_sha256") == required["source_sha256"], "Python oracle source hash mismatch")
        _check(record.get("support_files") == required["support_files"], "Python oracle support files mismatch")
        _check(source_lock.get(required["source_path"]) == required["source_sha256"], "Python source is not locked")
        source = _relative_owned(root, required["source_path"], "Python oracle source")
        _check(sha256(source) == required["source_sha256"], "Python oracle source bytes changed")
        _check(required["text"] == record.get("source_text"), "Python oracle evidence source text differs from the row candidate")
        _check(source.read_bytes().decode("utf-8") == record.get("source_text"), "Python oracle source must be the full candidate text")
        support_files = _support_files(root, record, f"Python oracle evidence {fixture_id}")
        _check(record.get("support_files") == support_files, "Python oracle support files differ from the row")
        for support in support_files:
            _check(source_lock.get(support["path"]) == support["sha256"], "Python oracle support file is not locked")
        tests = record.get("tests")
        _check(isinstance(tests, list) and tests, "Python oracle tests are missing")
        for test in tests:
            _check(isinstance(test, dict), "Python oracle test record is malformed")
            test_path = test.get("path")
            test_hash = test.get("sha256")
            _check(isinstance(test_path, str) and test_path.startswith(FIXTURE_DIR + "/"), "Python oracle test is outside proposed-fixtures")
            _check(isinstance(test_hash, str) and len(test_hash) == 64, "Python oracle test hash is missing")
            resolved = _relative_owned(root, test_path, "Python oracle test")
            _check(sha256(resolved) == test_hash == source_lock.get(test_path), "Python oracle test is not locked")
        expected_exit = record.get("expected_exit_status")
        _check(isinstance(expected_exit, int), "Python oracle expected exit status is missing")
        _check(record.get("expected_pass") is required["oracle_pass"], "Python oracle expected outcome mismatch")
        _check((expected_exit == 0) is required["oracle_pass"], "Python oracle expected exit contradicts expected outcome")
        _check(record.get("observed_pass") is required["oracle_pass"], "Python oracle observed outcome mismatch")
        _check(isinstance(record.get("exit_status"), int), "Python oracle exit status is missing")
        _check((record["exit_status"] == 0) is record["observed_pass"], "Python oracle observed exit contradicts observed outcome")
        _check(record["exit_status"] == expected_exit, "Python oracle observed exit status changed")
        _check(record.get("command") == _canonical_command([test["path"] for test in tests]), "Python oracle command changed")
        _check(record.get("fixed_fixture_only") is True, "Python oracle used a non-fixed fixture")
        canonical_records.append(record)
    digest = evidence.get("evidence_digest")
    canonical = json.dumps(canonical_records, sort_keys=True, separators=(",", ":"))
    _check(isinstance(digest, str) and digest == _sha_text(canonical), "Python oracle evidence digest changed")
    fresh = recompute_fixed_oracle(root, records, batch_size=oracle_batch_size)
    for fixture_id, observation in fresh.items():
        stored = records[fixture_id]
        for key in ("source_path", "source_sha256", "source_text", "tests", "support_files", "command", "expected_exit_status", "exit_status", "observed_pass", "fixed_fixture_only"):
            _check(stored.get(key) == observation.get(key), f"Python oracle recorded result differs from fresh fixed run: {fixture_id}")
    return {"path": str(evidence_path), "sha256": sha256(evidence_path), "candidates": len(records), "recomputed": True}


def _validate_final_approvals(root: Path, ledger: list[dict[str, Any]]) -> int:
    packet = _json(root / "reports" / "final-review-packet.json")
    entries = packet.get("entries") if isinstance(packet, dict) else None
    _check(isinstance(entries, list) and len(entries) == 60, "Python final review packet must contain 60 families")
    final_families = {item["family"] for item in ledger if item["split"] == "final"}
    approved_families = set()
    for entry in entries:
        _check(isinstance(entry, dict), "Python final review entry is malformed")
        family = entry.get("family")
        _check(family in final_families and family not in approved_families, "Python final review family changed")
        _check(entry.get("status") == "approved", "Python final family lacks controller approval")
        _check(isinstance(entry.get("reviewer_name"), str) and entry["reviewer_name"].strip(), "Python final approval must name its controller")
        _check(isinstance(entry.get("approval_sha256"), str) and len(entry["approval_sha256"]) == 64, "Python final approval digest is missing")
        approved_families.add(family)
    _check(approved_families == final_families, "Python final review packet does not cover every final family")
    return len(approved_families)


def audit_formal_python_corpus(root: Path, *, oracle_batch_size: int = 7) -> dict[str, Any]:
    """Audit a complete Python training corpus without creating evidence.

    This function never reads any previous audit certificate.  A successful
    result exists only in memory; callers may persist it as their own command
    output after every content check has passed.
    """
    _check(isinstance(oracle_batch_size, int) and not isinstance(oracle_batch_size, bool) and oracle_batch_size > 0,
           "Python oracle batch size must be a positive integer")
    root = Path(root)
    manifest = _validate_manifest(root)
    source_lock = _source_lock(root, root / "sources.lock.json")
    loaded = _rows(root)
    expected = _validate_rows(root, loaded, source_lock)
    ledger = _validate_ledger(root, loaded)
    evidence = _validate_evidence(root, expected, source_lock, oracle_batch_size=oracle_batch_size)
    reviewed = _validate_final_approvals(root, ledger)
    return {
        "status": "passed", "protocol": PROTOCOL, "domain": DOMAIN,
        "manifest_sha256": sha256(root / "manifest.json"),
        "source_lock_sha256": sha256(root / "sources.lock.json"),
        "family_ledger_sha256": sha256(root / "family-ledger.json"),
        "final_review_packet_sha256": sha256(root / "reports" / "final-review-packet.json"),
        "rows": ROW_COUNTS, "families": FAMILY_COUNTS,
        "reviewed_families": reviewed, "executable_oracle_evidence": evidence,
    }
