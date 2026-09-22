"""Offline SOM v1 frontend corpus access and approval-gated audit."""
from __future__ import annotations

import json
from collections import Counter
from pathlib import Path

from ..paths import read_json, sha256, write_json
from ..python_corpus import PythonCorpusError, audit_formal_python_corpus
from .som_config import DATA, DOMAINS, SPLITS, SOURCE_LOCK, corpus_path, corpus_source_lock, source_lock_sha256

MANIFEST = DATA / "manifest.json"
LEDGER = DATA / "family-ledger.json"
REVIEW_PACKET = DATA / "reports" / "final-review-packet.json"
AUDIT_JSON = DATA / "reports" / "family-audit-frontend.json"
AUDIT_MD = DATA / "reports" / "family-audit-frontend.md"
ORACLE_EVIDENCE = DATA / "reports" / "oracle-evidence.json"
COUNTS = {"train": 1200, "validation": 160, "calibration": 240, "final": 180}
PRESENCE = {"train": (600, 600), "validation": (107, 53), "calibration": (160, 80), "final": (120, 60)}


def _check(value: bool, message: str) -> None:
    if not value:
        raise ValueError(message)


def _paths(domain: str) -> dict[str, Path]:
    data = corpus_path(domain)
    return {
        "data": data,
        "manifest": data / "manifest.json",
        "ledger": data / "family-ledger.json",
        "review_packet": data / "reports" / "final-review-packet.json",
        "audit_json": data / "reports" / f"family-audit-{domain}.json",
        "audit_md": data / "reports" / f"family-audit-{domain}.md",
        "oracle_evidence": data / "reports" / "oracle-evidence.json",
        "source_lock": corpus_source_lock(domain),
    }


def rows(split: str, domain: str = "frontend") -> list[dict]:
    if split not in SPLITS or domain not in DOMAINS:
        raise ValueError("SOM corpus domain must be frontend or python.")
    path = corpus_path(domain) / f"{split}-{domain}.jsonl"
    return [json.loads(line) for line in path.read_text().splitlines() if line.strip()]


def prepare(domain: str = "frontend") -> dict:
    """Verify checked-in corpus bytes. Preparation never fetches or runs request code."""
    return verify(domain)


def executable_oracle_evidence() -> dict:
    """Validate candidate-level results from fixed local oracle execution.

    The checked-in per-family ``*.oracle.json`` files state an intended oracle.
    They are not execution evidence and must never satisfy this gate by
    themselves. The required report binds every candidate hash to a command,
    exit status, and result from a fixed repository fixture.
    """
    if not ORACLE_EVIDENCE.exists():
        raise ValueError("candidate-level executable oracle evidence is missing")
    evidence = read_json(ORACLE_EVIDENCE)
    _check(evidence.get("protocol") == "som-v1", "oracle evidence protocol changed")
    _check(evidence.get("status") == "passed", "oracle evidence is not passed")
    _check(evidence.get("executes_user_code") is False, "oracle evidence has an unsafe execution boundary")
    _check(evidence.get("family_ledger_sha256") == sha256(LEDGER), "oracle evidence does not bind the family ledger")
    result_map = evidence.get("candidates")
    _check(isinstance(result_map, dict), "oracle evidence has no candidate results")
    expected = {}
    for row in (item for split in SPLITS for item in rows(split)):
        for candidate in row["candidates"]:
            expected[candidate["id"]] = {
                "candidate_sha256": sha256_text(candidate["text"]),
                "oracle_pass": bool(candidate.get("oracle_pass")),
            }
    _check(set(result_map) == set(expected), "oracle evidence does not cover every candidate")
    for candidate_id, expected_result in expected.items():
        actual = result_map[candidate_id]
        _check(isinstance(actual, dict), "oracle result is malformed")
        _check(actual.get("candidate_sha256") == expected_result["candidate_sha256"], "oracle candidate hash mismatch")
        _check(actual.get("oracle_pass") is expected_result["oracle_pass"], "oracle candidate outcome mismatch")
        _check(isinstance(actual.get("command"), list) and actual["command"], "oracle command is missing")
        _check(actual.get("exit_code") == 0, "oracle command did not pass")
        _check(actual.get("fixed_fixture_only") is True, "oracle used a non-fixed fixture")
    return {"path": str(ORACLE_EVIDENCE), "sha256": sha256(ORACLE_EVIDENCE), "candidates": len(result_map)}


def sha256_text(value: str) -> str:
    import hashlib
    return hashlib.sha256(value.encode()).hexdigest()


def _python_audit() -> dict:
    """Audit the full Python corpus; never trust a checked-in audit certificate."""
    paths = _paths("python")
    if not paths["manifest"].exists():
        raise ValueError(f"Python corpus manifest is missing: {paths['manifest']}")
    try:
        return audit_formal_python_corpus(paths["data"])
    except PythonCorpusError as error:
        raise ValueError(str(error)) from error


def _audit(domain: str = "frontend") -> dict:
    if domain == "python":
        return _python_audit()
    manifest = read_json(MANIFEST)
    _check(manifest.get("protocol") == "som-v1", "corpus protocol changed")
    _check(
        manifest.get("corpus_status") == "ready_for_controller_review",
        "corpus is not ready for controller review",
    )
    ledger = read_json(LEDGER)
    _check(manifest.get("source_lock_sha256") == source_lock_sha256(), "source lock changed")
    _check(manifest.get("family_ledger_sha256") == sha256(LEDGER), "family ledger changed")
    oracle = executable_oracle_evidence()
    seen = set()
    lineage = set()
    for item in ledger:
        _check(item["family"] not in seen and item["lineage"] not in lineage, "family lineage leaks across splits")
        _check(len(item["candidate_provenance"]) == 7, "family does not have gold plus six near misses")
        _check(len({candidate["sha256"] for candidate in item["candidate_provenance"]}) == 7, "duplicate candidate in family")
        seen.add(item["family"]); lineage.add(item["lineage"])
    source_balance = {}
    capability_balance = {}
    for split in SPLITS:
        loaded = rows(split)
        present, missing = PRESENCE[split]
        _check(len(loaded) == COUNTS[split], f"{split} row count changed")
        _check(sum(not row["missing_correct_patch"] for row in loaded) == present, f"{split} present quota changed")
        _check(sum(row["missing_correct_patch"] for row in loaded) == missing, f"{split} missing quota changed")
        _check(manifest["files"].get(f"{split}-frontend.jsonl") == sha256(DATA / f"{split}-frontend.jsonl"), f"{split} bytes changed")
        for row in loaded:
            candidates = row["candidates"]
            _check(2 <= len(candidates) <= 7, "candidate count is outside 2-7")
            _check(len({candidate["id"] for candidate in candidates}) == len(candidates), "duplicate candidate id")
            passing = [candidate for candidate in candidates if candidate.get("oracle_pass")]
            if row["missing_correct_patch"]:
                _check(not passing and row["gold_candidate_id"] is None, "missing row includes a passing patch")
            else:
                _check(len(passing) == 1 and row["gold_candidate_id"] == passing[0]["id"], "gold oracle mismatch")
        families = [item for item in ledger if item["split"] == split]
        source_balance[split] = dict(Counter(item["source_kind"] for item in families))
        capability_balance[split] = dict(Counter(item["capability"] for item in families))
        _check(abs(source_balance[split].get("public", 0) - source_balance[split].get("authored", 0)) <= 1, "source balance changed")
    _check(capability_balance["final"] == {"dom": 20, "typescript": 20, "react": 20}, "final capability balance changed")
    packet = read_json(REVIEW_PACKET)
    entries = packet.get("entries", [])
    _check(len(entries) == 60, "final review packet must contain 60 families")
    approved = all(entry.get("status") == "approved" and entry.get("approval") for entry in entries)
    status = "passed" if approved else "pending_controller_review"
    return {"status": status, "protocol": "som-v1", "domain": "frontend", "rows": COUNTS,
            "source_balance": source_balance, "capability_balance": capability_balance,
            "review_packet": str(REVIEW_PACKET), "reviewed_families": sum(entry.get("status") == "approved" for entry in entries),
            "executable_oracle_evidence": oracle}


def audit(domain: str = "frontend") -> dict:
    if domain not in DOMAINS:
        raise ValueError("SOM audit supports frontend or python only.")
    paths = _paths(domain)
    try:
        result = _audit(domain)
    except (KeyError, TypeError, ValueError, FileNotFoundError) as error:
        result = {"status": "failed", "protocol": "som-v1", "domain": domain, "error": str(error)}
    write_json(paths["audit_json"], result)
    paths["audit_md"].write_text(f"# SOM {domain} family audit\n\nStatus: `{result['status']}`\n")
    return result


def verify(domain: str = "frontend") -> dict:
    result = audit(domain)
    if result["status"] == "failed":
        raise ValueError(result["error"])
    return result
