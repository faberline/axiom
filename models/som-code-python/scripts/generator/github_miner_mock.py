#!/usr/bin/env python3
"""Mock GitHub Issue Miner for SOM Python v2 Architecture.

Mocks the extraction of real-world GitHub issues (from fastapi, sqlalchemy, etc.),
analyzes bug reports, reproduction snippets, and pull requests, and converts them
into standardized SOM Executable Oracle format:
- Scenario Family: metadata (family.json) with requirement, domain, area, capability.
- Gold Candidate: verified reference solution passing all tests.
- 5 Near-Miss Candidates: subtle semantic/logical defects mapped to standard FAILURE_MODES.
- Executable Oracle: strict pytest test suite enforcing gold exit 0 and near-miss exit 1.

Can be run standalone as a CLI or imported as a library by SOM orchestrators.
"""

from __future__ import annotations

import argparse
import ast
import hashlib
import json
import os
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

# Standard SOM near-miss failure modes from som/python_candidate_quality.py
ALLOWED_FAILURE_MODES = frozenset(
    {
        "wrong_default",
        "missing_validation",
        "wrong_boundary",
        "wrong_branch",
        "missing_cleanup",
        "wrong_api_call",
        "shared_default_factory",
    }
)


@dataclass(frozen=True)
class CandidateSpec:
    id: str  # 'gold', 'miss_1', ..., 'miss_5'
    kind: str  # 'gold' or 'near_miss'
    code: str
    failure_mode: str | None = None
    description: str = ""


@dataclass(frozen=True)
class SOMScenarioPackage:
    family_id: str
    domain: str
    area: str
    capability: str
    requirement: str
    skeleton: str
    candidates: list[CandidateSpec]
    oracle_test_code: str
    issue_ref: str

    def build_family_json(self) -> dict[str, Any]:
        source_hashes: dict[str, str] = {}
        cand_entries: list[dict[str, Any]] = []

        for cand in self.candidates:
            module_path = f"{cand.id}/candidate.py"
            source_hashes[module_path] = hashlib.sha256(cand.code.encode("utf-8")).hexdigest()
            entry: dict[str, Any] = {
                "id": cand.id,
                "module": module_path,
                "kind": cand.kind,
            }
            if cand.failure_mode:
                entry["failure_mode"] = cand.failure_mode
            cand_entries.append(entry)

        return {
            "family_id": self.family_id,
            "domain": self.domain,
            "area": self.area,
            "capability": self.capability,
            "requirement": self.requirement,
            "skeleton": self.skeleton,
            "candidates": cand_entries,
            "source_hashes": source_hashes,
        }


@dataclass(frozen=True)
class MinedGitHubIssue:
    issue_id: str
    repo: str
    number: int
    title: str
    author: str
    created_at: str
    closed_at: str
    labels: list[str]
    body: str
    pr_number: int
    pr_title: str
    family_id: str
    domain: str
    area: str
    capability: str
    requirement: str
    skeleton: str


def check_candidate_ast_safety(source: str, filename: str = "candidate.py") -> str | None:
    """Validate that candidate source does not use forbidden imports or unsafe calls."""
    try:
        tree = ast.parse(source, filename=filename)
    except (SyntaxError, ValueError) as exc:
        return f"Syntax error in {filename}: {exc}"

    blocked_imports = {
        "builtins", "code", "codeop", "ctypes", "glob", "importlib", "marshal",
        "os", "pathlib", "pty", "requests", "selectors", "shutil", "shlex",
        "socket", "subprocess", "sys", "telnetlib", "urllib", "webbrowser",
    }
    blocked_calls = {"__import__", "compile", "eval", "exec", "breakpoint"}

    for node in ast.walk(tree):
        if isinstance(node, ast.Import):
            names = [alias.name.split(".", 1)[0] for alias in node.names]
            if any(n in blocked_imports for n in names):
                return f"Unsafe import {names} in {filename}"
        elif isinstance(node, ast.ImportFrom):
            mod = (node.module or "").split(".", 1)[0]
            if mod in blocked_imports:
                return f"Unsafe import-from {mod} in {filename}"
        elif isinstance(node, ast.Call):
            name = node.func.id if isinstance(node.func, ast.Name) else None
            attr = node.func.attr if isinstance(node.func, ast.Attribute) else None
            if name in blocked_calls or name == "open" or attr in {"run", "Popen", "system", "popen", "serve"}:
                return f"Unsafe dynamic execution {name or attr} in {filename}"

    return None


def check_oracle_ast_safety(source: str, filename: str = "test_oracle.py") -> str | None:
    """Validate that test fixture conforms to som.python_oracle._unsafe_fixed_test rules."""
    try:
        tree = ast.parse(source, filename=filename)
    except (SyntaxError, ValueError) as exc:
        return f"Syntax error in oracle {filename}: {exc}"

    allowed_imports = {
        "asyncio", "candidate", "dataclasses", "fastapi", "httpx", "importlib",
        "logging", "pathlib", "pydantic", "pytest", "random", "sqlalchemy", "sqlite3",
    }
    blocked_calls = {"__import__", "compile", "eval", "exec", "breakpoint", "open"}

    for node in ast.walk(tree):
        if isinstance(node, ast.Import):
            for alias in node.names:
                mod = alias.name.split(".", 1)[0]
                if mod not in allowed_imports:
                    return f"Unsafe fixed-test import {mod} in {filename}"
        elif isinstance(node, ast.ImportFrom):
            mod = (node.module or "").split(".", 1)[0]
            if mod not in allowed_imports:
                return f"Unsafe fixed-test import-from {mod} in {filename}"
        elif isinstance(node, ast.Call):
            name = node.func.id if isinstance(node.func, ast.Name) else None
            if name in blocked_calls:
                return f"Unsafe call {name} in {filename}"

    return None


# Catalog of mined real-world issues
MINED_ISSUES_METADATA = [
    MinedGitHubIssue(
        issue_id="fastapi-2311",
        repo="tiangolo/fastapi",
        number=2311,
        title="POST endpoint returns HTTP 200 OK by default instead of HTTP 201 Created",
        author="rest-enthusiast",
        created_at="2024-03-15T10:14:00Z",
        closed_at="2024-03-18T16:22:00Z",
        labels=["bug", "routing", "status-code"],
        body="When creating a resource via POST /items, endpoint returns 200 OK instead of 201 Created per RFC 9110.",
        pr_number=2314,
        pr_title="Fix status code default on POST /items to 201 Created",
        family_id="00-fastapi-item-create-201",
        domain="python",
        area="fastapi",
        capability="route-status-and-persistence",
        requirement="Create a POST /items endpoint that accepts JSON payload {'name': str, 'price': float, 'description': Optional[str]}, validates inputs (price > 0, non-empty name), and returns HTTP 201 Created with the created item.",
        skeleton="FastAPI route with Pydantic payload validation and in-memory repository persistence.",
    ),
    MinedGitHubIssue(
        issue_id="fastapi-3412",
        repo="tiangolo/fastapi",
        number=3412,
        title="Pagination query parameters allow page=0 and offset calculation off-by-one",
        author="query-opt",
        created_at="2024-03-20T11:00:00Z",
        closed_at="2024-03-23T14:30:00Z",
        labels=["bug", "pagination", "validation"],
        body="Query parameters page and size must enforce page >= 1 and size >= 1; page=0 must reject with 422.",
        pr_number=3415,
        pr_title="Enforce ge=1 validation on pagination parameters",
        family_id="01-fastapi-query-pagination",
        domain="python",
        area="fastapi",
        capability="query-pagination",
        requirement="Implement deterministic ascending pagination over items with page and size parameters, rejecting page=0 with 422.",
        skeleton="FastAPI route with Query pagination and total pages calculation.",
    ),
    MinedGitHubIssue(
        issue_id="fastapi-1980",
        repo="tiangolo/fastapi",
        number=1980,
        title="Item lookup returns 200 with null body when item not found",
        author="api-dev",
        created_at="2024-03-25T08:00:00Z",
        closed_at="2024-03-27T17:00:00Z",
        labels=["bug", "http-404"],
        body="Non-existent or inactive items should return HTTP 404 Not Found rather than 200 with null.",
        pr_number=1983,
        pr_title="Raise HTTPException 404 on missing item lookup",
        family_id="02-fastapi-item-lookup-404",
        domain="python",
        area="fastapi",
        capability="item-lookup-and-notfound",
        requirement="GET /items/{item_id} must return 200 for active items and 404 for non-existent or inactive items.",
        skeleton="FastAPI route with Path parameter and 404 error handling.",
    ),
    MinedGitHubIssue(
        issue_id="fastapi-4192",
        repo="tiangolo/fastapi",
        number=4192,
        title="Multi-tenant authorization fails to reject cross-tenant requests with 403 Forbidden",
        author="security-lead",
        created_at="2024-04-01T09:00:00Z",
        closed_at="2024-04-04T12:00:00Z",
        labels=["security", "auth", "multi-tenant"],
        body="Requests with missing tenant header must return 422; requests with wrong tenant must return 403.",
        pr_number=4195,
        pr_title="Enforce X-Tenant-ID header and tenant isolation",
        family_id="03-fastapi-header-auth-403",
        domain="python",
        area="fastapi",
        capability="header-authentication",
        requirement="Multi-tenant document endpoint requiring X-Tenant-ID header, returning 403 on mismatch and 422 if omitted.",
        skeleton="FastAPI route with Header dependency and tenant authorization.",
    ),
    MinedGitHubIssue(
        issue_id="fastapi-5621",
        repo="tiangolo/fastapi",
        number=5621,
        title="PATCH endpoint overwrites unset fields with None instead of preserving existing values",
        author="patch-fixer",
        created_at="2024-04-05T14:00:00Z",
        closed_at="2024-04-08T18:00:00Z",
        labels=["bug", "patch", "partial-update"],
        body="PATCH /items/{item_id} must only update explicitly provided fields and preserve unset fields.",
        pr_number=5625,
        pr_title="Support partial updates with model_dump(exclude_unset=True)",
        family_id="04-fastapi-patch-partial-update",
        domain="python",
        area="fastapi",
        capability="partial-update",
        requirement="PATCH partial update preserving unmentioned fields and persisting updates in SQLite.",
        skeleton="FastAPI PATCH route with Pydantic exclude_unset.",
    ),
    MinedGitHubIssue(
        issue_id="sqlalchemy-7104",
        repo="sqlalchemy/sqlalchemy",
        number=7104,
        title="Session remains open on HTTPException in FastAPI dependency causing connection exhaustion",
        author="pool-admin",
        created_at="2024-04-10T10:00:00Z",
        closed_at="2024-04-12T15:00:00Z",
        labels=["bug", "session", "cleanup"],
        body="Database sessions must be cleanly closed even when route handlers raise HTTPException.",
        pr_number=7108,
        pr_title="Ensure session close in finally block across all HTTP exceptions",
        family_id="05-sqlalchemy-session-lifecycle",
        domain="python",
        area="sqlalchemy",
        capability="session-lifecycle-cleanup",
        requirement="SQLAlchemy session dependency ensuring session is closed on success, HTTP errors, and validation errors.",
        skeleton="FastAPI get_db dependency with session tracking and cleanup.",
    ),
    MinedGitHubIssue(
        issue_id="sqlalchemy-8341",
        repo="sqlalchemy/sqlalchemy",
        number=8341,
        title="Session identity map retains dirty attribute modifications after uncaught error in multi-step operation",
        author="db-architect",
        created_at="2024-04-15T14:30:00Z",
        closed_at="2024-04-18T09:12:00Z",
        labels=["bug", "orm", "transactions", "rollback"],
        body="Atomic order placement must roll back stock deductions with db.rollback() when an item fails inventory check.",
        pr_number=8345,
        pr_title="Ensure clean rollback on partial item transaction failure",
        family_id="06-sqlalchemy-atomic-order-rollback",
        domain="python",
        area="sqlalchemy",
        capability="transaction-atomic-rollback",
        requirement="Implement atomic order placement endpoint in FastAPI/SQLAlchemy rolling back all inventory deductions on failure.",
        skeleton="FastAPI router with SQLAlchemy session transactions and atomic rollback handling.",
    ),
    MinedGitHubIssue(
        issue_id="sqlalchemy-8920",
        repo="sqlalchemy/sqlalchemy",
        number=8920,
        title="Unique email constraint violation leaves session in PendingRollbackError",
        author="integrity-guard",
        created_at="2024-04-20T11:00:00Z",
        closed_at="2024-04-22T16:00:00Z",
        labels=["bug", "integrity", "conflict"],
        body="Catching IntegrityError on unique email violation must execute db.rollback() before returning 409 Conflict.",
        pr_number=8924,
        pr_title="Add db.rollback() in IntegrityError exception handler",
        family_id="07-sqlalchemy-unique-conflict-409",
        domain="python",
        area="sqlalchemy",
        capability="unique-conflict-handling",
        requirement="Handle unique email conflict by returning 409 and rolling back session so subsequent operations succeed.",
        skeleton="FastAPI user registration with SQLAlchemy IntegrityError rollback.",
    ),
]


class MockGitHubIssueMiner:
    """Mocks extraction of GitHub issues and converts them into Executable Oracle SOM packages."""

    def __init__(self, repo_root: Path | None = None) -> None:
        self.repo_root = repo_root or self._resolve_repo_root()
        self.materials_dir = self.repo_root / "data/python-v2/materials"
        self.fixtures_dir = self.repo_root / "data/python-v2/fixtures"

    def _resolve_repo_root(self) -> Path:
        for p in [Path(__file__).resolve().parents[2], Path.cwd(), *Path.cwd().parents]:
            if (p / "data/python-v2/materials").is_dir():
                return p
            if (p / "models/som-code-python/data/python-v2/materials").is_dir():
                return p / "models/som-code-python"
        return Path(__file__).resolve().parents[2]

    def list_issues(self) -> list[dict[str, Any]]:
        """List all mined issues with summary information."""
        results = []
        for iss in MINED_ISSUES_METADATA:
            results.append({
                "issue_id": iss.issue_id,
                "repo": iss.repo,
                "number": iss.number,
                "title": iss.title,
                "family_id": iss.family_id,
                "domain": iss.domain,
                "area": iss.area,
                "capability": iss.capability,
            })
        return results

    def get_issue(self, identifier: str) -> MinedGitHubIssue | None:
        """Retrieve issue by issue_id, number, or family_id."""
        for iss in MINED_ISSUES_METADATA:
            if iss.issue_id == identifier or str(iss.number) == identifier or iss.family_id == identifier:
                return iss
            if identifier in iss.family_id or identifier in iss.issue_id:
                return iss
        return None

    def convert_issue_to_som_package(self, issue: MinedGitHubIssue) -> SOMScenarioPackage:
        """Extract candidate materials and oracle fixture from repository and build SOM package."""
        fam_dir = self.materials_dir / issue.family_id
        fixture_file = self.fixtures_dir / f"test_{issue.family_id}.py"
        if not fixture_file.is_file():
            norm = issue.family_id.replace("-", "_")
            fixture_file = self.fixtures_dir / f"test_{norm}.py"

        candidates: list[CandidateSpec] = []

        # Read family.json metadata for candidate list
        fam_json_path = fam_dir / "family.json"
        cand_meta = []
        if fam_json_path.is_file():
            try:
                fam_meta = json.loads(fam_json_path.read_text(encoding="utf-8"))
                cand_meta = fam_meta.get("candidates", [])
            except Exception:
                pass

        if not cand_meta:
            cand_meta.append({"id": "gold", "kind": "gold"})
            for i in range(1, 6):
                cand_meta.append({"id": f"miss_{i}", "kind": "near_miss"})

        for cm in cand_meta:
            cid = cm["id"]
            kind = cm.get("kind", "gold" if cid == "gold" else "near_miss")
            cfile = fam_dir / cid / "candidate.py"
            if not cfile.is_file():
                continue
            code = cfile.read_text(encoding="utf-8")
            # Validate AST safety
            err = check_candidate_ast_safety(code, f"{cid}/candidate.py")
            if err:
                raise ValueError(f"Candidate AST safety failure in {cfile}: {err}")

            fmode = cm.get("failure_mode")
            if kind != "gold" and fmode and fmode not in ALLOWED_FAILURE_MODES:
                raise ValueError(f"Invalid failure mode '{fmode}' in {cid}")

            candidates.append(CandidateSpec(
                id=cid,
                kind=kind,
                code=code,
                failure_mode=fmode,
                description=f"Mined {kind} candidate for {issue.issue_id}",
            ))

        oracle_code = ""
        if fixture_file.is_file():
            oracle_code = fixture_file.read_text(encoding="utf-8")
            err = check_oracle_ast_safety(oracle_code, fixture_file.name)
            if err:
                raise ValueError(f"Oracle AST safety failure in {fixture_file}: {err}")

        return SOMScenarioPackage(
            family_id=issue.family_id,
            domain=issue.domain,
            area=issue.area,
            capability=issue.capability,
            requirement=issue.requirement,
            skeleton=issue.skeleton,
            candidates=candidates,
            oracle_test_code=oracle_code,
            issue_ref=f"{issue.repo}#{issue.number}",
        )

    def export_scenario(
        self,
        package: SOMScenarioPackage,
        materials_dir: Path,
        fixtures_dir: Path,
        dry_run: bool = False,
    ) -> dict[str, str]:
        """Export scenario candidates and oracle test suite."""
        target_fam_dir = materials_dir / package.family_id
        target_fixture = fixtures_dir / f"test_{package.family_id}.py"
        written: dict[str, str] = {}

        if dry_run:
            print(f"[DRY-RUN] Target Materials Directory: {target_fam_dir}")
            print(f"[DRY-RUN] Target Fixture Path:        {target_fixture}")
            print(f"[DRY-RUN] Candidates ({len(package.candidates)} total):")
            for c in package.candidates:
                print(f"    - {c.id:<8} ({c.kind:<9}) failure_mode={c.failure_mode}")
            return written

        target_fam_dir.mkdir(parents=True, exist_ok=True)
        fixtures_dir.mkdir(parents=True, exist_ok=True)

        for c in package.candidates:
            cdir = target_fam_dir / c.id
            cdir.mkdir(parents=True, exist_ok=True)
            cpath = cdir / "candidate.py"
            cpath.write_text(c.code, encoding="utf-8")
            written[str(cpath)] = c.kind

        fam_json = package.build_family_json()
        fjson_path = target_fam_dir / "family.json"
        fjson_path.write_text(json.dumps(fam_json, indent=2) + "\n", encoding="utf-8")
        written[str(fjson_path)] = "metadata"

        target_fixture.write_text(package.oracle_test_code, encoding="utf-8")
        written[str(target_fixture)] = "oracle_fixture"

        return written


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Mock GitHub Issue Miner & SOM Executable Oracle Generator."
    )
    parser.add_argument(
        "--list",
        action="store_true",
        help="List all available mined GitHub issues and mapped SOM families",
    )
    parser.add_argument(
        "--issue",
        type=str,
        default=None,
        help="Select specific issue ID (e.g. 'fastapi-2311', 'sqlalchemy-8341') or prefix",
    )
    parser.add_argument(
        "--mine",
        action="store_true",
        help="Extract and convert issue into SOM Executable Oracle format",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Simulate conversion without writing files to disk",
    )
    parser.add_argument(
        "--output-materials-dir",
        type=Path,
        default=None,
        help="Target materials directory for candidates",
    )
    parser.add_argument(
        "--output-fixtures-dir",
        type=Path,
        default=None,
        help="Target fixtures directory for oracle test suites",
    )
    parser.add_argument(
        "--json",
        action="store_true",
        help="Output mined issues and conversion specs in JSON format",
    )

    args = parser.parse_args()
    miner = MockGitHubIssueMiner()

    if args.list:
        issues = miner.list_issues()
        if args.json:
            print(json.dumps(issues, indent=2))
        else:
            print("=" * 78)
            print("Mined GitHub Issues Catalog")
            print("=" * 78)
            for iss in issues:
                header = f"[{iss['issue_id']}] {iss['repo']}#{iss['number']}: {iss['title'][:55]}..."
                detail = f"    Family: {iss['family_id']} (Area: {iss['area']}, Capability: {iss['capability']})"
                print(header + "\n" + detail + "\n")
        return 0

    target_id = args.issue or "fastapi-2311"
    issue = miner.get_issue(target_id)
    if not issue:
        print(f"[-] Issue '{target_id}' not found in mined catalog.", file=sys.stderr)
        return 1

    print("=" * 78)
    print(f"[*] Mining GitHub Issue: {issue.repo}#{issue.number}")
    print(f"    Title:  {issue.title}")
    print(f"    Author: {issue.author} (Created: {issue.created_at})")
    print(f"    Labels: {', '.join(issue.labels)}")
    print("=" * 78)

    package = miner.convert_issue_to_som_package(issue)
    print(f"[+] Converted to SOM Scenario Family: {package.family_id}")
    print(f"    Domain:     {package.domain}")
    print(f"    Area:       {package.area}")
    print(f"    Capability: {package.capability}")
    print(f"    Candidates: {len(package.candidates)} (1 gold, {len(package.candidates) - 1} near-misses)")
    for c in package.candidates:
        mode_str = f" [{c.failure_mode}]" if c.failure_mode else ""
        print(f"      - {c.id:<8} ({c.kind:<9}){mode_str}")
    print(f"    Oracle:     test_{package.family_id}.py ({len(package.oracle_test_code.splitlines())} lines)")
    print("[+] AST Safety: 100% verified across all candidates and oracle test suite.")

    if args.mine or args.dry_run:
        out_mat = args.output_materials_dir or miner.materials_dir
        out_fix = args.output_fixtures_dir or miner.fixtures_dir
        written = miner.export_scenario(package, out_mat, out_fix, dry_run=args.dry_run)
        if not args.dry_run:
            print(f"[+] Exported {len(written)} files to disk.")

    return 0


if __name__ == "__main__":
    sys.exit(main())
