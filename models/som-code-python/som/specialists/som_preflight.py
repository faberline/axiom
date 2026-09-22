"""Fail-closed SOM training and evaluation preflight.

This module reads repository-owned assets only. It never evaluates request text
or candidate code. Fixed-oracle execution is recorded separately by the corpus
oracle runner and bound here by hash.
"""
from __future__ import annotations

from ..paths import read_json, sha256
from . import som_data
from .som_config import MIXED_CONTROL, verify_som_seed

def _control_hashes() -> dict:
    manifest_path = MIXED_CONTROL / "manifest.json"
    manifest = read_json(manifest_path)
    actual = {}
    for name, entry in manifest.get("files", {}).items():
        path = MIXED_CONTROL / "rows" / name
        digest = sha256(path)
        if digest != entry.get("sha256"):
            raise ValueError(f"mixed-control hash mismatch: {name}")
        actual[name] = digest
    if manifest.get("asset") != "som-mixed-control-v1" or len(actual) != 8:
        raise ValueError("mixed-control manifest is incomplete")
    return {"manifest_sha256": sha256(manifest_path), "rows": actual}


def _oracle_evidence() -> dict:
    return som_data.executable_oracle_evidence()


def preflight(domain: str = "frontend") -> dict:
    """Require immutable seed/control hashes, oracle evidence, and approvals."""
    errors = []
    values = {}
    # `_audit` is read-only. Preflight may run alongside a controller audit, so
    # it must not contend for the audit-report destination.
    checks = [("seed", verify_som_seed)]
    if domain == "frontend":
        checks += [("mixed_control", _control_hashes), ("oracle", _oracle_evidence), ("audit", som_data._audit)]
    else:
        checks += [("audit", lambda: som_data._audit(domain))]
    for name, check in checks:
        try:
            values[name] = check()
        except (FileNotFoundError, KeyError, TypeError, ValueError) as error:
            errors.append(f"{name}: {error}")
    if values.get("audit", {}).get("status") != "passed":
        message = "audit: final review requires named approval for all 60 families" if domain == "frontend" else "audit: corpus audit is not passed"
        errors.append(message)
    return {"status": "passed" if not errors else "failed", "protocol": "som-v1", "checks": values, "errors": errors,
            "executes_user_code": False}


def require_ready(domain: str = "frontend") -> dict:
    result = preflight(domain)
    if result["status"] != "passed":
        raise ValueError("SOM preflight failed; smoke, training, and evaluation are blocked: " + "; ".join(result["errors"]))
    return result
