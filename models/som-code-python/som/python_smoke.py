"""Fail-closed contract for the 32-row Python SOM smoke corpus.

The smoke corpus is deliberately separate from the formal Python corpus.  It
can prove that the frozen Q/V LoRA mechanics can learn from 32 reviewed fixed
fixtures.  It cannot authorise formal training or evaluation.

Only repository-owned fixtures below ``data/som/python-v1/smoke`` are ever
read here.  This module has no request argument and never runs request code.
"""
from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any

from .paths import sha256
from .python_corpus import PythonCorpusError, _canonical_command, _support_files, recompute_fixed_oracle


PROTOCOL = "som-v1"
DOMAIN = "python"
CORPUS_KIND = "smoke"
ROW_COUNT = 32
FIXTURE_DIR = "proposed-fixtures"


class PythonSmokeError(ValueError):
    """The separate Python smoke corpus does not meet its frozen contract."""


def _check(value: bool, message: str) -> None:
    if not value:
        raise PythonSmokeError(message)


def _json(path: Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise PythonSmokeError(f"required Python smoke file is missing: {path}") from exc
    except json.JSONDecodeError as exc:
        raise PythonSmokeError(f"invalid JSON in Python smoke file {path}: {exc}") from exc


def _owned_file(root: Path, value: Any, label: str) -> Path:
    _check(isinstance(value, str) and value and not Path(value).is_absolute(), f"{label} must be a relative path")
    path = (root / value).resolve()
    try:
        path.relative_to(root.resolve())
    except ValueError as exc:
        raise PythonSmokeError(f"{label} escapes the Python smoke root") from exc
    _check(path.is_file(), f"{label} is missing: {value}")
    return path


def _text_hash(value: str) -> str:
    return hashlib.sha256(value.encode("utf-8")).hexdigest()


def smoke_root(corpus_root: Path) -> Path:
    """Return the distinct smoke root without accepting a formal root as smoke."""
    return Path(corpus_root) / "smoke"


def _manifest(root: Path) -> dict[str, Any]:
    manifest = _json(root / "manifest.json")
    _check(isinstance(manifest, dict), "Python smoke manifest must be an object")
    _check(manifest.get("protocol") == PROTOCOL and manifest.get("domain") == DOMAIN,
           "Python smoke protocol changed")
    _check(manifest.get("corpus_kind") == CORPUS_KIND,
           "Python formal corpus cannot masquerade as a smoke corpus")
    _check(manifest.get("corpus_status") == "ready_for_controller_review",
           "Python smoke corpus is not ready for controller review")
    _check(manifest.get("fixture_root") == FIXTURE_DIR, "Python smoke fixture layout changed")
    files = manifest.get("files")
    _check(isinstance(files, dict), "Python smoke manifest has no hashes")
    required = (
        "smoke-python.jsonl", "family-ledger.json", "sources.lock.json",
        "reports/oracle-evidence.json", "reports/controller-approval.json",
    )
    for relative in required:
        digest = files.get(relative)
        _check(isinstance(digest, str) and len(digest) == 64,
               f"Python smoke manifest has no hash for {relative}")
        _check(digest == sha256(_owned_file(root, relative, "Python smoke manifest path")),
               f"Python smoke bytes changed: {relative}")
    _check(manifest.get("row_sha256") == files["smoke-python.jsonl"], "Python smoke row manifest binding changed")
    _check(manifest.get("family_ledger_sha256") == files["family-ledger.json"],
           "Python smoke ledger manifest binding changed")
    _check(manifest.get("source_lock_sha256") == files["sources.lock.json"],
           "Python smoke source-lock manifest binding changed")
    return manifest


def _source_lock(root: Path) -> dict[str, str]:
    lock = _json(root / "sources.lock.json")
    _check(isinstance(lock, dict) and lock.get("protocol") == PROTOCOL and lock.get("domain") == DOMAIN,
           "Python smoke source lock protocol changed")
    files = lock.get("files")
    _check(isinstance(files, dict) and files, "Python smoke source lock has no file hashes")
    result: dict[str, str] = {}
    for relative, entry in files.items():
        _check(isinstance(relative, str) and isinstance(entry, dict), "Python smoke source lock entry is malformed")
        digest = entry.get("sha256")
        _check(isinstance(digest, str) and len(digest) == 64, "Python smoke source lock hash is malformed")
        _check(digest == sha256(_owned_file(root, relative, "Python smoke source lock path")),
               f"Python smoke source lock hash changed: {relative}")
        result[relative] = digest
    return result


def _rows(root: Path) -> list[dict[str, Any]]:
    path = root / "smoke-python.jsonl"
    try:
        rows = [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line.strip()]
    except FileNotFoundError as exc:
        raise PythonSmokeError(f"Python smoke rows are missing: {path}") from exc
    except json.JSONDecodeError as exc:
        raise PythonSmokeError(f"invalid Python smoke JSONL: {exc}") from exc
    _check(len(rows) == ROW_COUNT and all(isinstance(row, dict) for row in rows),
           "Python smoke must have exactly 32 object rows")
    return rows


def _candidate_count_histograms(rows: list[dict[str, Any]]) -> tuple[dict[int, int], dict[int, int]]:
    """Return present and missing candidate-count histograms for the frozen rows."""
    present: dict[int, int] = {}
    missing: dict[int, int] = {}
    for row in rows:
        candidates = row.get("candidates")
        if not isinstance(candidates, list):
            continue
        histogram = missing if bool(row.get("missing_correct_patch")) else present
        count = len(candidates)
        histogram[count] = histogram.get(count, 0) + 1
    return present, missing


def _validate_rows(root: Path, rows: list[dict[str, Any]], lock: dict[str, str]) -> dict[str, dict[str, Any]]:
    ids: set[str] = set(); families: set[str] = set(); lineages: set[str] = set(); fixture_ids: set[str] = set()
    present = missing = 0
    expected: dict[str, dict[str, Any]] = {}
    for row in rows:
        row_id, family, lineage = row.get("id"), row.get("family"), row.get("lineage")
        _check(all(isinstance(item, str) and item for item in (row_id, family, lineage)),
               "Python smoke row lacks id, family, or lineage")
        _check(row_id not in ids and family not in families and lineage not in lineages,
               "Python smoke row, family, or lineage is duplicated")
        ids.add(row_id); families.add(family); lineages.add(lineage)
        candidates = row.get("candidates")
        _check(isinstance(candidates, list) and 2 <= len(candidates) <= 7,
               "Python smoke candidate count is outside 2-7")
        candidate_ids = [candidate.get("id") for candidate in candidates if isinstance(candidate, dict)]
        _check(len(candidate_ids) == len(candidates) and all(isinstance(item, str) and item for item in candidate_ids)
               and len(candidate_ids) == len(set(candidate_ids)), "Python smoke candidate IDs are invalid")
        passing = []
        for candidate in candidates:
            fixture_id = candidate.get("fixture_id")
            source_path, source_digest = candidate.get("source_path"), candidate.get("source_sha256")
            _check(isinstance(fixture_id, str) and fixture_id and fixture_id not in fixture_ids,
                   "Python smoke fixture candidate is reused")
            fixture_ids.add(fixture_id)
            _check(isinstance(source_path, str) and source_path.startswith(FIXTURE_DIR + "/"),
                   "Python smoke candidate source is outside proposed-fixtures")
            _check(isinstance(source_digest, str) and len(source_digest) == 64,
                   "Python smoke candidate source hash is missing")
            source = _owned_file(root, source_path, "Python smoke candidate source")
            _check(source_digest == sha256(source) == lock.get(source_path),
                   "Python smoke candidate source is not locked")
            text = candidate.get("text")
            _check(isinstance(text, str) and text == source.read_text(encoding="utf-8"),
                   "Python smoke candidate text differs from its fixed source")
            try:
                support_files = _support_files(root, candidate, f"Python smoke candidate {fixture_id}")
            except PythonCorpusError as error:
                raise PythonSmokeError(str(error)) from error
            for support in support_files:
                _check(lock.get(support["path"]) == support["sha256"],
                       "Python smoke support is not locked")
            expected[fixture_id] = {"source_path": source_path, "source_sha256": source_digest,
                                    "support_files": support_files,
                                    "oracle_pass": bool(candidate.get("oracle_pass")), "text": text,
                                    "family": family}
            if candidate.get("oracle_pass"):
                passing.append(candidate)
        if bool(row.get("missing_correct_patch")):
            missing += 1
            _check(not passing and row.get("gold_candidate_id") is None,
                   "Python smoke missing row includes a passing patch")
        else:
            present += 1
            _check(len(passing) == 1 and row.get("gold_candidate_id") == passing[0]["id"],
                   "Python smoke gold oracle mismatch")
    _check((present, missing) == (16, 16), "Python smoke present/missing quota must be 16/16")
    present_histogram, missing_histogram = _candidate_count_histograms(rows)
    _check(present_histogram == missing_histogram,
           "Python smoke present and missing candidate-count histograms must match")
    return expected


def _ledger(root: Path, rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    ledger = _json(root / "family-ledger.json")
    _check(isinstance(ledger, list) and len(ledger) == ROW_COUNT, "Python smoke ledger must have 32 families")
    row_map = {row["family"]: row for row in rows}
    families: set[str] = set(); lineages: set[str] = set()
    for item in ledger:
        _check(isinstance(item, dict), "Python smoke ledger item is malformed")
        family, lineage = item.get("family"), item.get("lineage")
        _check(isinstance(family, str) and isinstance(lineage, str) and family in row_map,
               "Python smoke ledger family is unknown")
        _check(family not in families and lineage not in lineages,
               "Python smoke ledger family or lineage is duplicated")
        _check(lineage == row_map[family]["lineage"], "Python smoke ledger lineage differs from its row")
        provenance = item.get("candidate_provenance")
        _check(isinstance(provenance, list) and 2 <= len(provenance) <= 7,
               "Python smoke candidate provenance is incomplete")
        digests = [entry.get("sha256") for entry in provenance if isinstance(entry, dict)]
        _check(len(digests) == len(provenance) and len(digests) == len(set(digests))
               and all(isinstance(value, str) and len(value) == 64 for value in digests),
               "Python smoke candidate provenance hashes are invalid")
        families.add(family); lineages.add(lineage)
    _check(families == set(row_map), "Python smoke ledger does not cover every family")
    return ledger


def _evidence(root: Path, expected: dict[str, dict[str, Any]], lock: dict[str, str]) -> dict[str, Any]:
    evidence_path = root / "reports" / "oracle-evidence.json"
    evidence = _json(evidence_path)
    _check(isinstance(evidence, dict) and evidence.get("protocol") == PROTOCOL and evidence.get("domain") == DOMAIN
           and evidence.get("corpus_kind") == CORPUS_KIND, "Python smoke oracle evidence protocol changed")
    _check(evidence.get("executes_user_code") is False and evidence.get("fixture_root") == FIXTURE_DIR,
           "Python smoke oracle boundary changed")
    records = evidence.get("candidates")
    _check(isinstance(records, dict) and set(records) == set(expected),
           "Python smoke oracle evidence does not cover every candidate")
    canonical: list[dict[str, Any]] = []
    for fixture_id, required in expected.items():
        record = records[fixture_id]
        _check(isinstance(record, dict), "Python smoke oracle candidate record is malformed")
        _check(record.get("source_path") == required["source_path"] and record.get("source_sha256") == required["source_sha256"],
               "Python smoke oracle source differs from its row")
        _check(record.get("support_files") == required["support_files"],
               "Python smoke oracle support files differ from its row")
        _check(record.get("source_text") == required["text"], "Python smoke oracle text differs from its row")
        tests = record.get("tests")
        _check(isinstance(tests, list) and tests, "Python smoke oracle tests are missing")
        for test in tests:
            _check(isinstance(test, dict) and isinstance(test.get("path"), str) and test["path"].startswith(FIXTURE_DIR + "/"),
                   "Python smoke oracle test is outside proposed-fixtures")
            _check(test.get("sha256") == lock.get(test["path"]) == sha256(_owned_file(root, test["path"], "Python smoke test")),
                   "Python smoke test is not locked")
        expected_exit = record.get("expected_exit_status")
        _check(isinstance(expected_exit, int) and record.get("expected_pass") is required["oracle_pass"],
               "Python smoke oracle expected outcome changed")
        _check((expected_exit == 0) is required["oracle_pass"] and record.get("observed_pass") is required["oracle_pass"]
               and record.get("exit_status") == expected_exit,
               "Python smoke oracle observed outcome changed")
        _check(record.get("command") == _canonical_command([test["path"] for test in tests])
               and record.get("fixed_fixture_only") is True, "Python smoke oracle command changed")
        canonical.append(record)
    _check(evidence.get("evidence_digest") == _text_hash(json.dumps(canonical, sort_keys=True, separators=(",", ":"))),
           "Python smoke oracle evidence digest changed")
    try:
        fresh = recompute_fixed_oracle(root, records)
    except PythonCorpusError as error:
        raise PythonSmokeError(str(error)) from error
    for fixture_id, observation in fresh.items():
        stored = records[fixture_id]
        for key in ("source_path", "source_sha256", "source_text", "tests", "support_files", "command", "expected_exit_status",
                    "exit_status", "observed_pass", "fixed_fixture_only"):
            _check(stored.get(key) == observation.get(key),
                   f"Python smoke oracle recorded result differs from fresh fixed run: {fixture_id}")
    return {"path": str(evidence_path), "sha256": sha256(evidence_path), "candidates": len(records), "recomputed": True}


def _approvals(root: Path, ledger: list[dict[str, Any]]) -> int:
    packet = _json(root / "reports" / "controller-approval.json")
    entries = packet.get("entries") if isinstance(packet, dict) else None
    _check(isinstance(entries, list) and len(entries) == ROW_COUNT,
           "Python smoke controller approval must contain 32 families")
    required = {entry["family"] for entry in ledger}
    approved: set[str] = set()
    for entry in entries:
        _check(isinstance(entry, dict), "Python smoke approval entry is malformed")
        family = entry.get("family")
        _check(family in required and family not in approved and entry.get("status") == "approved",
               "Python smoke family lacks controller approval")
        _check(isinstance(entry.get("reviewer_name"), str) and entry["reviewer_name"].strip(),
               "Python smoke approval must name its controller")
        _check(isinstance(entry.get("approval_sha256"), str) and len(entry["approval_sha256"]) == 64,
               "Python smoke approval digest is missing")
        approved.add(family)
    _check(approved == required, "Python smoke approvals do not cover every family")
    return len(approved)


def audit_python_smoke(corpus_root: Path) -> dict[str, Any]:
    """Audit and recompute a 32-row smoke corpus; never trust a prior report."""
    root = smoke_root(corpus_root)
    manifest = _manifest(root)
    lock = _source_lock(root)
    rows = _rows(root)
    expected = _validate_rows(root, rows, lock)
    ledger = _ledger(root, rows)
    evidence = _evidence(root, expected, lock)
    approved = _approvals(root, ledger)
    return {
        "status": "passed", "protocol": PROTOCOL, "domain": DOMAIN, "corpus_kind": CORPUS_KIND,
        "smoke_manifest_sha256": sha256(root / "manifest.json"),
        "source_lock_sha256": sha256(root / "sources.lock.json"),
        "family_ledger_sha256": sha256(root / "family-ledger.json"),
        "row_sha256": sha256(root / "smoke-python.jsonl"), "rows": ROW_COUNT,
        "present": 16, "missing": 16, "reviewed_families": approved,
        "executable_oracle_evidence": evidence,
    }


def smoke_rows(corpus_root: Path) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    """Return only rows that passed the independent smoke contract."""
    audit = audit_python_smoke(corpus_root)
    return _rows(smoke_root(corpus_root)), audit
