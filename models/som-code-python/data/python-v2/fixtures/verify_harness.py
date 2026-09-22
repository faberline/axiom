#!/usr/bin/env python3
"""Verification Harness for SOM Python v2 Dataset Scenarios.

Discovers scenario families in materials/, maps each to its corresponding
oracle test suite in fixtures/, and verifies the Executable Oracle principle:
- Gold candidate: MUST exit 0 (passes all behavioral tests)
- Near-miss candidates (miss_1..miss_5): MUST exit 1 (fails at least one test assertion, not syntax/collection errors)
- A near miss that declares ``caught_by`` in family.json MUST fail exactly those
  tests, so the curated record and the executed oracle cannot drift apart
- A family that declares ``oracle`` MUST name the fixture file this harness ran

Exits 0 if and only if 100% of gold candidates pass (exit 0), 100% of near-miss
candidates fail test assertions (exit 1) with the declared ``caught_by`` set, and
every ``oracle`` declaration matches the fixture used.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import signal
import subprocess
import tempfile
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any


@dataclass(frozen=True)
class CandidateRunResult:
    family_id: str
    candidate_id: str
    kind: str  # "gold" or "near_miss"
    returncode: int
    expected_exit_zero: bool
    passed_oracle_contract: bool
    stdout: str
    stderr: str
    failure_mode: str | None = None
    caught_by_expected: list[str] | None = None
    caught_by_actual: list[str] | None = None


_FAILED_LINE_RE = re.compile(r"^(?:FAILED|ERROR) (\S+)", re.MULTILINE)


def failed_test_names(stdout: str) -> list[str]:
    """Extract the sorted, de-duplicated test function names pytest reported as failed."""
    names: set[str] = set()
    for node_id in _FAILED_LINE_RE.findall(stdout):
        name = node_id.rsplit("::", 1)[-1]
        names.add(name.split("[", 1)[0])
    return sorted(names)


def find_python_executable(repo_root: Path) -> str:
    """Locate the project virtualenv python executable if present, else sys.executable."""
    venv_python = repo_root / ".venv" / "bin" / "python"
    if venv_python.is_file() and os.access(venv_python, os.X_OK):
        return str(venv_python)
    # Search upwards from repo_root if .venv is not found directly at repo_root
    for parent in [repo_root, *repo_root.parents]:
        v = parent / ".venv" / "bin" / "python"
        if v.is_file() and os.access(v, os.X_OK):
            return str(v)
    return sys.executable


def matches_family_filter(term: str, family_name: str) -> bool:
    """Check if a filter term matches a family name without false substring collisions."""
    # Exact match: "00-fastapi-item-create-201" or normalized "00_fastapi_item_create_201"
    if term == family_name or term == family_name.replace("-", "_"):
        return True
    # Prefix match: "01", "01-", "01_"
    if family_name.startswith(term) and (term.endswith("-") or term.endswith("_")):
        return True
    if family_name.startswith(f"{term}-") or family_name.startswith(f"{term}_"):
        return True
    # Single digit shorthand, e.g. "1" matches "01-"
    if term.isdigit() and len(term) < 2:
        if family_name.startswith(f"{int(term):02d}-"):
            return True
    # Word/token boundary match on hyphen/underscore boundaries
    # E.g. "create" matches "00-fastapi-item-create-201", but "01" will NOT match "201"
    pattern = rf"(^|[-_]){re.escape(term)}([-_]|$)"
    if re.search(pattern, family_name):
        return True
    return False



def default_materials_dir() -> Path:
    cand = Path(__file__).resolve().parent.parent / "materials"
    if cand.is_dir():
        return cand
    for parent in Path(__file__).resolve().parents:
        cand = parent / "models/som-code-python/data/python-v2/materials"
        if cand.is_dir():
            return cand
        cand = parent / "data/python-v2/materials"
        if cand.is_dir():
            return cand
    return cand


def default_fixtures_dir() -> Path:
    cand = Path(__file__).resolve().parent
    if (cand / "test_00_fastapi_item_create_201.py").is_file():
        return cand
    for parent in Path(__file__).resolve().parents:
        cand = parent / "models/som-code-python/data/python-v2/fixtures"
        if cand.is_dir():
            return cand
        cand = parent / "data/python-v2/fixtures"
        if cand.is_dir():
            return cand
    return cand


def default_repo_root() -> Path:
    for parent in Path(__file__).resolve().parents:
        if (parent / "models/som-code-python/.venv").is_dir():
            return parent / "models/som-code-python"
        if (parent / ".venv").is_dir() and (parent / "som").is_dir():
            return parent
    return Path(__file__).resolve().parents[3]


def discover_families(
    materials_dir: Path,
    filter_terms: list[str] | None = None,
) -> list[Path]:
    """Find all family directories in materials_dir, optionally filtered."""
    if not materials_dir.is_dir():
        return []

    family_dirs = [
        d for d in materials_dir.iterdir()
        if d.is_dir() and (d / "family.json").is_file()
    ]
    family_dirs.sort(key=lambda p: p.name)

    if not filter_terms:
        return family_dirs

    matched: list[Path] = []
    for fdir in family_dirs:
        for term in filter_terms:
            if matches_family_filter(term, fdir.name):
                matched.append(fdir)
                break
    return matched


def find_fixture_test(fixtures_dir: Path, family_name: str) -> Path | None:
    """Find the matching test fixture file for a given family name."""
    # Try exact match: test_<family_name>.py
    exact_candidate = fixtures_dir / f"test_{family_name}.py"
    if exact_candidate.is_file():
        return exact_candidate

    # Try underscore normalized: test_<family_name_with_underscores>.py
    normalized_name = family_name.replace("-", "_")
    norm_candidate = fixtures_dir / f"test_{normalized_name}.py"
    if norm_candidate.is_file():
        return norm_candidate

    # Try matching by numeric prefix, e.g. "00" matches "test_00_*.py"
    prefix = family_name.split("-")[0]
    for file in fixtures_dir.glob(f"test_{prefix}_*.py"):
        if file.is_file():
            return file

    return None



# Pytest plugin code injected into each candidate run to guarantee per-test
# DB isolation, dependency override clearing, and monkeypatch cleanup.
_HARNESS_ISOLATION_PLUGIN_CODE = """import pytest
import candidate

@pytest.fixture(autouse=True)
def _som_harness_test_isolation():
    # --- PRE-TEST ISOLATION ---
    if hasattr(candidate, "app") and hasattr(candidate.app, "dependency_overrides"):
        candidate.app.dependency_overrides.clear()

    if hasattr(candidate, "reset_db"):
        try:
            candidate.reset_db()
        except Exception:
            pass
    elif hasattr(candidate, "Base") and hasattr(candidate, "engine"):
        try:
            candidate.Base.metadata.drop_all(bind=candidate.engine)
            candidate.Base.metadata.create_all(bind=candidate.engine)
        except Exception:
            pass

    original_attrs = {}
    if hasattr(candidate, "__dict__"):
        for k, v in list(candidate.__dict__.items()):
            if not k.startswith("__"):
                original_attrs[k] = v

    yield

    # --- POST-TEST TEARDOWN & RECOVERY ---
    if hasattr(candidate, "app") and hasattr(candidate.app, "dependency_overrides"):
        candidate.app.dependency_overrides.clear()

    if hasattr(candidate, "engine") and hasattr(candidate.engine, "dispose"):
        try:
            candidate.engine.dispose()
        except Exception:
            pass

    if hasattr(candidate, "reset_db"):
        try:
            candidate.reset_db()
        except Exception:
            pass
    elif hasattr(candidate, "Base") and hasattr(candidate, "engine"):
        try:
            candidate.Base.metadata.drop_all(bind=candidate.engine)
            candidate.Base.metadata.create_all(bind=candidate.engine)
        except Exception:
            pass

    if hasattr(candidate, "__dict__"):
        current_keys = set(candidate.__dict__.keys())
        for k in current_keys - set(original_attrs.keys()):
            if not k.startswith("__"):
                try:
                    delattr(candidate, k)
                except Exception:
                    pass
        for k, v in original_attrs.items():
            if candidate.__dict__.get(k) is not v:
                try:
                    setattr(candidate, k, v)
                except Exception:
                    pass
"""


def sanitize_environment(
    candidate_dir: Path,
    repo_root: Path,
    temp_dir: Path,
) -> dict[str, str]:
    """Construct a clean, sanitized environment for running candidate tests.

    Strips host database connection strings, configures isolated temporary directories,
    disables bytecode caching and pytest autoloading, and sets deterministic hash seeds.
    """
    env = os.environ.copy()

    candidate_resolved = str(candidate_dir.resolve())
    repo_root_resolved = str(repo_root.resolve())
    existing_pythonpath = env.get("PYTHONPATH", "")
    env["PYTHONPATH"] = (
        f"{candidate_resolved}:{repo_root_resolved}:{str(temp_dir)}:{existing_pythonpath}".strip(":")
    )

    db_env_keys = [
        "DATABASE_URL", "SQLALCHEMY_DATABASE_URI", "DB_URL", "SQLITE_URL",
        "POSTGRES_URL", "MYSQL_URL", "PGDATABASE", "PGUSER", "PGHOST",
        "MYSQL_DATABASE", "MYSQL_USER", "REDIS_URL",
    ]
    for key in db_env_keys:
        env.pop(key, None)
    env["DATABASE_URL"] = "sqlite:///:memory:"

    env["TMPDIR"] = str(temp_dir)
    env["TEMP"] = str(temp_dir)
    env["TMP"] = str(temp_dir)

    env["PYTEST_DISABLE_PLUGIN_AUTOLOAD"] = "1"
    env["PYTHONHASHSEED"] = "0"
    env["PYTHONDONTWRITEBYTECODE"] = "1"
    env["PYTHONBREAKPOINT"] = "0"
    env.pop("PYTEST_ADDOPTS", None)

    return env


def run_candidate_test(
    python_exe: str,
    repo_root: Path,
    candidate_dir: Path,
    test_file: Path,
    timeout: int = 45,
) -> tuple[int, str, str]:
    """Execute pytest with candidate_dir prepended to PYTHONPATH in an isolated workspace."""
    with tempfile.TemporaryDirectory(prefix="som_candidate_run_") as scratch_dir_str:
        scratch_dir = Path(scratch_dir_str)
        cache_dir = scratch_dir / ".pytest_cache"
        cache_dir.mkdir(parents=True, exist_ok=True)

        isolation_plugin_path = scratch_dir / "_harness_isolation_plugin.py"
        isolation_plugin_path.write_text(_HARNESS_ISOLATION_PLUGIN_CODE, encoding="utf-8")

        env = sanitize_environment(
            candidate_dir=candidate_dir,
            repo_root=repo_root,
            temp_dir=scratch_dir,
        )

        cmd = [
            python_exe,
            "-m",
            "pytest",
            "-q",
            "--disable-warnings",
            "-p",
            "no:cacheprovider",
            "-o",
            f"cache_dir={cache_dir}",
            "-p",
            "_harness_isolation_plugin",
            str(test_file.resolve()),
        ]

        proc = None
        try:
            proc = subprocess.Popen(
                cmd,
                cwd=str(scratch_dir),
                env=env,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                start_new_session=True,
            )
            stdout, stderr = proc.communicate(timeout=timeout)
            return proc.returncode, stdout, stderr
        except subprocess.TimeoutExpired:
            if proc is not None:
                try:
                    os.killpg(os.getpgid(proc.pid), signal.SIGKILL)
                except Exception:
                    proc.kill()
                stdout, stderr = proc.communicate()
            else:
                stdout, stderr = "", ""
            return 124, stdout or "", f"TimeoutExpired after {timeout}s: " + (stderr or "")
        except FileNotFoundError as exc:
            return 127, "", f"Python executable not found: {python_exe} ({exc})"
        except Exception as exc:
            return 128, "", f"Execution failure: {exc}"


def execute_family(
    family_dir: Path,
    fixtures_dir: Path,
    repo_root: Path,
    python_exe: str,
    verbose: bool = False,
) -> list[CandidateRunResult]:
    """Run all candidates for a single family and collect results."""
    family_json_path = family_dir / "family.json"
    results: list[CandidateRunResult] = []

    try:
        with open(family_json_path, encoding="utf-8") as f:
            metadata: dict[str, Any] = json.load(f)
    except Exception as exc:
        print(f"[-] Error reading {family_json_path}: {exc}", file=sys.stderr)
        return results

    family_id = metadata.get("family_id", family_dir.name)
    test_file = find_fixture_test(fixtures_dir, family_dir.name)
    if not test_file:
        print(
            f"[-] No matching test file found in {fixtures_dir} for family {family_dir.name}",
            file=sys.stderr,
        )
        return results

    declared_oracle = metadata.get("oracle")
    if declared_oracle is not None and declared_oracle != f"fixtures/{test_file.name}":
        print(
            f"[-] family.json declares oracle {declared_oracle!r} but the harness runs fixtures/{test_file.name}",
            file=sys.stderr,
        )
        results.append(
            CandidateRunResult(
                family_id=family_id,
                candidate_id="family.json",
                kind="declaration",
                returncode=0,
                expected_exit_zero=False,
                passed_oracle_contract=False,
                stdout="",
                stderr=f"oracle declares {declared_oracle!r}, harness ran fixtures/{test_file.name}",
            )
        )
        return results

    candidates_meta = metadata.get("candidates", [])
    if not candidates_meta:
        # Fallback to scanning directory
        entries = []
        if (family_dir / "gold" / "candidate.py").is_file():
            entries.append({"id": "gold", "module": "gold/candidate.py", "kind": "gold"})
        for i in range(1, 6):
            miss_id = f"miss_{i}"
            if (family_dir / miss_id / "candidate.py").is_file():
                entries.append({"id": miss_id, "module": f"{miss_id}/candidate.py", "kind": "near_miss"})
        candidates_meta = entries

    for cand in candidates_meta:
        cand_id = cand.get("id", "unknown")
        kind = cand.get("kind", "gold" if cand_id == "gold" else "near_miss")
        module_rel = cand.get("module", f"{cand_id}/candidate.py")
        failure_mode = cand.get("failure_mode")
        candidate_file = family_dir / module_rel
        candidate_dir = candidate_file.parent

        if not candidate_file.is_file():
            print(f"[-] Candidate file missing: {candidate_file}", file=sys.stderr)
            results.append(
                CandidateRunResult(
                    family_id=family_id,
                    candidate_id=cand_id,
                    kind=kind,
                    returncode=127,
                    expected_exit_zero=(kind == "gold"),
                    passed_oracle_contract=False,
                    stdout="",
                    stderr=f"Candidate file missing: {candidate_file}",
                    failure_mode=failure_mode,
                )
            )
            continue

        is_gold = (kind == "gold")
        code, stdout, stderr = run_candidate_test(
            python_exe=python_exe,
            repo_root=repo_root,
            candidate_dir=candidate_dir,
            test_file=test_file,
        )

        # Oracle Contract:
        # Gold must exit 0 (all behavioral tests pass)
        # Near-miss must exit 1 (pytest assertion failure on flaw)
        # Exit code 2 indicates syntax/collection error (defect)
        # Exit code 124 indicates timeout (defect)
        # A declared caught_by must equal the set of tests that actually failed
        expected_caught = cand.get("caught_by") if not is_gold else None
        actual_caught = failed_test_names(stdout) if not is_gold else None
        if is_gold:
            passed_contract = code == 0
        else:
            passed_contract = code == 1 and (
                expected_caught is None or sorted(expected_caught) == actual_caught
            )

        results.append(
            CandidateRunResult(
                family_id=family_id,
                candidate_id=cand_id,
                kind=kind,
                returncode=code,
                expected_exit_zero=is_gold,
                passed_oracle_contract=passed_contract,
                stdout=stdout,
                stderr=stderr,
                failure_mode=failure_mode,
                caught_by_expected=expected_caught,
                caught_by_actual=actual_caught,
            )
        )

    return results


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Verify Executable Oracle contracts for SOM Python v2 dataset."
    )
    parser.add_argument(
        "families",
        nargs="*",
        help="Optional family identifiers or filter substrings (e.g. 00, 01, 00-fastapi-item-create-201)",
    )
    parser.add_argument(
        "-f",
        "--family",
        action="append",
        dest="family_filters",
        help="Filter specific family name/prefix (can be specified multiple times)",
    )
    parser.add_argument(
        "-v",
        "--verbose",
        action="store_true",
        help="Print detailed test output even on successful assertions",
    )
    parser.add_argument(
        "--materials-dir",
        type=Path,
        default=default_materials_dir(),
        help="Directory containing family scenario materials",
    )
    parser.add_argument(
        "--fixtures-dir",
        type=Path,
        default=default_fixtures_dir(),
        help="Directory containing pytest fixture test suites",
    )
    parser.add_argument(
        "--repo-root",
        type=Path,
        default=default_repo_root(),
        help="Repository root directory for som-code-python",
    )
    parser.add_argument(
        "--python-exe",
        type=str,
        default=None,
        help="Python executable to execute pytest (defaults to repo .venv/bin/python)",
    )

    args = parser.parse_args()

    materials_dir: Path = args.materials_dir.resolve()
    fixtures_dir: Path = args.fixtures_dir.resolve()
    repo_root: Path = args.repo_root.resolve()
    python_exe = args.python_exe or find_python_executable(repo_root)

    filter_terms: list[str] = []
    if args.families:
        filter_terms.extend(args.families)
    if args.family_filters:
        filter_terms.extend(args.family_filters)

    family_dirs = discover_families(materials_dir, filter_terms or None)

    if not family_dirs:
        print(f"[!] No matching family directories found in {materials_dir}")
        if filter_terms:
            print(f"    Filters applied: {filter_terms}")
        return 1

    print("=" * 78)
    print("SOM Python v2 Executable Oracle Verification Harness")
    print(f"Materials:   {materials_dir}")
    print(f"Fixtures:    {fixtures_dir}")
    print(f"Python:      {python_exe}")
    print(f"Families:    {len(family_dirs)} discovered")
    print("=" * 78)

    all_results: list[CandidateRunResult] = []

    for fdir in family_dirs:
        print(f"\nEvaluating Family: {fdir.name}")
        print("-" * 60)
        results = execute_family(
            family_dir=fdir,
            fixtures_dir=fixtures_dir,
            repo_root=repo_root,
            python_exe=python_exe,
            verbose=args.verbose,
        )
        all_results.extend(results)

        for res in results:
            tag = "PASS" if res.passed_oracle_contract else "FAIL"
            color_prefix = "\033[32m" if res.passed_oracle_contract else "\033[31m"
            color_suffix = "\033[0m"

            if res.kind == "declaration":
                expectation_str = "oracle names the fixture run"
                status_str = "oracle mismatch"
            elif res.kind == "gold":
                expectation_str = "exit code == 0"
                status_str = f"exit code {res.returncode}"
            else:
                mode_str = f" [{res.failure_mode}]" if res.failure_mode else ""
                expectation_str = f"exit code == 1{mode_str}"
                if res.returncode == 0:
                    status_str = "exit code 0 (passed all tests)"
                elif res.returncode == 1 and not res.passed_oracle_contract:
                    status_str = "exit code 1, caught_by drift"
                elif res.returncode == 1:
                    status_str = "exit code 1"
                elif res.returncode == 2:
                    status_str = "exit code 2 (collection/import error)"
                elif res.returncode == 124:
                    status_str = "exit code 124 (timeout)"
                elif res.returncode == 127:
                    status_str = "exit code 127 (missing)"
                else:
                    status_str = f"exit code {res.returncode}"

            print(
                f"  [{color_prefix}{tag}{color_suffix}] {res.candidate_id:<8} ({res.kind:<9}) "
                f"-> {status_str:<14} (expected {expectation_str})"
            )

            if res.caught_by_expected is not None and res.caught_by_actual != sorted(res.caught_by_expected):
                print(f"     [caught_by] declared: {sorted(res.caught_by_expected)}")
                print(f"     [caught_by] actual:   {res.caught_by_actual}")
            if not res.passed_oracle_contract or args.verbose:
                if res.stdout.strip():
                    print("     [stdout]:")
                    for line in res.stdout.strip().splitlines()[-10:]:
                        print(f"       {line}")
                if res.stderr.strip():
                    print("     [stderr]:")
                    for line in res.stderr.strip().splitlines()[-10:]:
                        print(f"       {line}")

    # Summary Statistics
    total_candidates = len(all_results)
    gold_results = [r for r in all_results if r.kind == "gold"]
    miss_results = [r for r in all_results if r.kind == "near_miss"]
    declaration_results = [r for r in all_results if r.kind == "declaration"]

    gold_passed = sum(1 for r in gold_results if r.passed_oracle_contract)
    miss_passed = sum(1 for r in miss_results if r.passed_oracle_contract)
    total_passed = gold_passed + miss_passed

    print("\n" + "=" * 78)
    print("Verification Summary")
    print("=" * 78)
    print(f"Total Families Evaluated:        {len(family_dirs)}")
    print(f"Total Candidates Evaluated:      {total_candidates}")
    print(
        f"Gold Candidates (Exit == 0):     {gold_passed}/{len(gold_results)} "
        f"({100.0 * gold_passed / (len(gold_results) or 1):.1f}%)"
    )
    print(
        f"Near-Miss Candidates (Exit == 1): {miss_passed}/{len(miss_results)} "
        f"({100.0 * miss_passed / (len(miss_results) or 1):.1f}%)"
    )

    all_pass = (
        len(gold_results) > 0
        and gold_passed == len(gold_results)
        and miss_passed == len(miss_results)
        and not declaration_results
    )

    if all_pass:
        print("\n\033[32m[RESULT: SUCCESS] 100% of gold passed (exit 0) and 100% of near-misses failed test assertions (exit 1).\033[0m")
        return 0
    else:
        print("\n\033[31m[RESULT: FAILURE] Oracle verification contracts violated!\033[0m")
        failed_entries = [r for r in all_results if not r.passed_oracle_contract]
        for r in failed_entries:
            if r.kind == "declaration":
                print(f"  - Declaration Defect: {r.stderr} for {r.family_id}")
            elif r.returncode == 1 and r.caught_by_expected is not None:
                print(f"  - Curation Drift: Near-miss {r.candidate_id} ({r.failure_mode}) declares caught_by {sorted(r.caught_by_expected)} but failed {r.caught_by_actual} for {r.family_id}")
            elif r.kind == "gold":
                print(f"  - Defect: Gold candidate failed for {r.family_id} (exit code {r.returncode})")
            elif r.returncode == 0:
                print(f"  - False Negative: Near-miss {r.candidate_id} ({r.failure_mode}) PASSED oracle for {r.family_id} (exit code 0)")
            elif r.returncode == 2:
                print(f"  - Collection Defect: Near-miss {r.candidate_id} ({r.failure_mode}) crashed pytest collection/import (exit code 2) for {r.family_id}")
            elif r.returncode == 124:
                print(f"  - Timeout Defect: Near-miss {r.candidate_id} ({r.failure_mode}) timed out (exit code 124) for {r.family_id}")
            else:
                print(f"  - Unexpected Exit Defect: Near-miss {r.candidate_id} ({r.failure_mode}) exited with code {r.returncode} for {r.family_id}")
        return 1


if __name__ == "__main__":
    sys.exit(main())
