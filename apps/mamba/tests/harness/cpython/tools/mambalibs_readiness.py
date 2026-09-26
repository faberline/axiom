#!/usr/bin/env python3.12
"""Mambalibs/native-kit readiness accounting for #714.

This is a replacement-readiness report, not a mambalibs runner. It keeps
Mamba-native library evidence separate from pure-Python package management and
from CPython C-extension compatibility, while still naming the third-party
ecosystem gaps that native kits are expected to cover.
"""

from __future__ import annotations

import argparse
import json
import re
import tomllib
from collections import Counter
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from platform_readiness import EXIT_NOT_READY, MAMBA_DIR, repo_rel


REPO_ROOT = MAMBA_DIR.parents[1]
FIXTURES_DIR = MAMBA_DIR / "tests" / "mambalibs" / "fixtures"
MAMBALIBS_DIR = MAMBA_DIR / "mambalibs"
MAMBALIBS_README = MAMBALIBS_DIR / "README.md"
FORCE_LINK = MAMBA_DIR / "src" / "pkgmanage" / "builder" / "force_link.rs"


@dataclass(frozen=True)
class NativeKit:
    id: str
    namespace: str
    implementation_paths: tuple[str, ...]
    import_gate_family: str | None
    third_party_replacements: tuple[str, ...]
    future_runtime_integration: bool = False


NATIVE_KITS: tuple[NativeKit, ...] = (
    NativeKit("array", "mambalibs.array", ("apps/mamba/mambalibs/arraykit",), "array_import_gate", ("numpy",)),
    NativeKit("fetch", "mambalibs.fetch", (), "fetch_import_gate", ("requests", "httpx", "aiohttp"), True),
    NativeKit("frame", "mambalibs.frame", ("apps/mamba/crates/frame",), "frame_import_gate", ("pandas",), True),
    NativeKit("grid", "mambalibs.grid", ("apps/jet/crates/grid",), "grid_import_gate", (), True),
    NativeKit("learn", "mambalibs.learn", ("crates/cclab-learn", "apps/mamba/mambalibs/scikit"), "learn_import_gate", ("scikit-learn",), True),
    NativeKit("log", "mambalibs.log", ("apps/mamba/crates/log-mamba", "apps/mamba/crates/log"), "log_import_gate", ("loguru", "structlog"), True),
    NativeKit("media", "mambalibs.media", ("apps/mamba/mambalibs/mediakit",), "media_import_gate", (), True),
    NativeKit("pg", "mambalibs.pg", ("apps/mamba/mambalibs/pgkit",), "pg_import_gate", ("psycopg",), True),
    NativeKit("plot", "mambalibs.plot", ("apps/mamba/mambalibs/plotkit",), "plot_import_gate", (), True),
    NativeKit("schema", "mambalibs.schema", ("apps/mamba/crates/schema-mamba", "apps/mamba/crates/schema"), "schema_import_gate", ("pydantic", "marshmallow", "jsonschema"), True),
    NativeKit("sci", "mambalibs.sci", ("apps/mamba/mambalibs/scikit",), "sci_import_gate", ("numpy", "scipy"), True),
    NativeKit("text", "mambalibs.text", ("crates/cclab-text",), "text_import_gate", (), True),
    NativeKit("http", "mambalibs.http", ("apps/mamba/mambalibs/httpkit",), None, ("requests", "httpx", "fastapi", "aiohttp")),
    NativeKit("di", "mambalibs.di", ("apps/mamba/mambalibs/dikit",), None, ("fastapi",)),
    NativeKit("dataclasses", "mambalibs.dataclasses", ("apps/mamba/crates/schema-mamba", "apps/mamba/crates/schema"), None, ("pydantic", "marshmallow", "jsonschema")),
    NativeKit("queue", "mambalibs.queue", ("apps/mamba/mambalibs/queuekit",), None, ("celery", "rq")),
    NativeKit("crypto", "mambalibs.crypto", ("apps/mamba/mambalibs/cryptokit",), None, ("cryptography", "pyopenssl")),
    NativeKit("mongo", "mambalibs.mongo", ("apps/mamba/mambalibs/mongokit",), None, ()),
)

REQUIRED_SUPPORT_STATUSES = ("pass", "xfail", "blocker")
GENERIC_MODE2_FAMILIES = {
    "from_mambalibs_import",
    "multiple_mambalibs_import",
    "mambalibs_mode2_end_to_end_build_and_import",
    "async_export_blocker",
    "pyi_stub_generation",
    "type_roundtrip",
    "rust_error_propagation",
}


def repo_path(path: str) -> Path:
    return REPO_ROOT / path


def rel_path(path: Path) -> str:
    try:
        return repo_rel(path)
    except ValueError:
        return path.resolve().relative_to(REPO_ROOT.resolve()).as_posix()


def load_toml(path: Path) -> tuple[dict[str, Any], dict[str, str] | None]:
    try:
        return tomllib.loads(path.read_text(encoding="utf-8")), None
    except tomllib.TOMLDecodeError as exc:
        return {}, {
            "kind": "mambalibs_manifest_parse_error",
            "path": rel_path(path),
            "error": str(exc),
            "owner_refs": ["#714"],
            "owned": True,
        }


def fixture_manifests() -> tuple[list[dict[str, Any]], list[dict[str, str]]]:
    manifests: list[dict[str, Any]] = []
    errors: list[dict[str, str]] = []
    if not FIXTURES_DIR.exists():
        return manifests, [
            {
                "kind": "mambalibs_fixtures_dir_missing",
                "path": rel_path(FIXTURES_DIR),
                "owner_refs": ["#714"],
                "owned": True,
            }
        ]
    for path in sorted(FIXTURES_DIR.glob("*/manifest.toml")):
        parsed, error = load_toml(path)
        if error:
            errors.append(error)
            continue
        support = parsed.get("support_status", {})
        binding = parsed.get("binding", {})
        blocked_case = parsed.get("blocked_case", {})
        manifests.append(
            {
                "path": rel_path(path),
                "fixture": parsed.get("fixture", ""),
                "family": parsed.get("family", ""),
                "issue": parsed.get("issue", parsed.get("umbrella_issue", "")),
                "parent_issue": parsed.get("parent_issue", parsed.get("umbrella_issue", "")),
                "library": parsed.get("library")
                or (binding.get("library") if isinstance(binding, dict) else "")
                or "",
                "import_statement": (
                    binding.get("import_statement", "")
                    if isinstance(binding, dict)
                    else ""
                ),
                "support_status": (
                    support.get("current_status", "")
                    if isinstance(support, dict)
                    else ""
                ),
                "allowed_statuses": (
                    support.get("allowed_values", [])
                    if isinstance(support, dict)
                    else []
                ),
                "linked_blocker_issue": (
                    blocked_case.get("linked_blocker_issue", "")
                    if isinstance(blocked_case, dict)
                    else ""
                ),
            }
        )
    return manifests, errors


def expected_kits() -> tuple[set[str], list[dict[str, str]]]:
    if not FORCE_LINK.exists():
        return set(), [
            {
                "kind": "force_link_file_missing",
                "path": rel_path(FORCE_LINK),
                "owner_refs": ["#714"],
                "owned": True,
            }
        ]
    text = FORCE_LINK.read_text(encoding="utf-8", errors="replace")
    match = re.search(r"EXPECTED_KITS:\s*&\[&str\]\s*=\s*&\[(?P<body>.*?)\];", text, re.S)
    if not match:
        return set(), [
            {
                "kind": "expected_kits_parse_error",
                "path": rel_path(FORCE_LINK),
                "owner_refs": ["#714"],
                "owned": True,
            }
        ]
    return set(re.findall(r'"([^"]+)"', match.group("body"))), []


def rust_tests_for_paths(paths: tuple[str, ...]) -> list[str]:
    tests: list[str] = []
    for rel in paths:
        root = repo_path(rel)
        if not root.exists():
            continue
        for path in sorted(root.glob("tests/*.rs")):
            tests.append(rel_path(path))
    return tests


def manifest_by_family(manifests: list[dict[str, Any]]) -> dict[str, dict[str, Any]]:
    by_family: dict[str, dict[str, Any]] = {}
    for manifest in manifests:
        family = manifest.get("family")
        if isinstance(family, str) and family:
            by_family[family] = manifest
    return by_family


def kit_report(
    kit: NativeKit,
    by_family: dict[str, dict[str, Any]],
    registered: set[str],
) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    existing_paths = [path for path in kit.implementation_paths if repo_path(path).exists()]
    missing_paths = [path for path in kit.implementation_paths if not repo_path(path).exists()]
    gate = by_family.get(kit.import_gate_family or "")
    gate_status = gate.get("support_status", "") if gate else ""
    registered_in_runtime = kit.namespace in registered
    rust_tests = rust_tests_for_paths(kit.implementation_paths)
    blockers: list[dict[str, Any]] = []

    if not kit.implementation_paths:
        blockers.append(
            {
                "kind": "native_kit_implementation_path_undecided",
                "kit": kit.id,
                "namespace": kit.namespace,
                "owner_refs": ["#714"],
                "owned": True,
                "reason": "native-kit requirement has no concrete implementation path yet",
            }
        )
    for path in missing_paths:
        blockers.append(
            {
                "kind": "native_kit_implementation_path_missing",
                "kit": kit.id,
                "namespace": kit.namespace,
                "path": path,
                "owner_refs": ["#714"],
                "owned": True,
                "reason": "declared native-kit backing path does not exist",
            }
        )

    if kit.import_gate_family and not gate:
        blockers.append(
            {
                "kind": "native_kit_import_gate_missing",
                "kit": kit.id,
                "namespace": kit.namespace,
                "family": kit.import_gate_family,
                "owner_refs": ["#714"],
                "owned": True,
                "reason": "required native-kit import gate is not present in tests/mambalibs",
            }
        )
    if kit.import_gate_family and gate_status not in REQUIRED_SUPPORT_STATUSES:
        blockers.append(
            {
                "kind": "native_kit_import_gate_status_missing",
                "kit": kit.id,
                "namespace": kit.namespace,
                "family": kit.import_gate_family,
                "owner_refs": ["#714"],
                "owned": True,
                "reason": "native-kit import gate lacks pass/xfail/blocker support status",
            }
        )
    if gate_status in {"xfail", "blocker"}:
        blockers.append(
            {
                "kind": f"native_kit_import_gate_{gate_status}",
                "kit": kit.id,
                "namespace": kit.namespace,
                "family": kit.import_gate_family,
                "issue": gate.get("issue", ""),
                "linked_blocker_issue": gate.get("linked_blocker_issue", ""),
                "owner_refs": ["#714"],
                "owned": True,
                "reason": "native-kit import/callable surface is explicitly not pass-ready",
            }
        )
    if not registered_in_runtime:
        blockers.append(
            {
                "kind": "native_kit_not_force_link_registered",
                "kit": kit.id,
                "namespace": kit.namespace,
                "owner_refs": ["#714"],
                "owned": True,
                "reason": "namespace is not listed in pkgmanage::builder::force_link EXPECTED_KITS",
            }
        )
    if kit.future_runtime_integration:
        blockers.append(
            {
                "kind": "future_runtime_or_jit_integration",
                "kit": kit.id,
                "namespace": kit.namespace,
                "owner_refs": ["#714"],
                "owned": True,
                "reason": "kit is tracked as replacement-path native coverage but needs runtime/JIT integration before it can count as pass-ready",
            }
        )

    if blockers:
        status = "blocker" if gate_status in {"xfail", "blocker"} or kit.future_runtime_integration else "fail"
    else:
        status = "pass"

    return (
        {
            "id": kit.id,
            "namespace": kit.namespace,
            "status": status,
            "implementation_paths": list(kit.implementation_paths),
            "existing_implementation_paths": existing_paths,
            "missing_implementation_paths": missing_paths,
            "registered_in_runtime": registered_in_runtime,
            "import_gate_family": kit.import_gate_family,
            "import_gate_path": gate.get("path", "") if gate else "",
            "import_gate_status": gate_status or "missing",
            "third_party_replacements": list(kit.third_party_replacements),
            "future_runtime_integration": kit.future_runtime_integration,
            "rust_test_count": len(rust_tests),
            "rust_tests": rust_tests,
            "blocker_count": len(blockers),
        },
        blockers,
    )


def generic_mode2_status(manifests: list[dict[str, Any]]) -> dict[str, Any]:
    selected = [
        manifest
        for manifest in manifests
        if manifest.get("family") in GENERIC_MODE2_FAMILIES
        or str(manifest.get("family", "")).startswith("mambalibs_mode2")
    ]
    statuses = Counter(
        manifest["support_status"] or "declared"
        for manifest in selected
    )
    blockers = [
        {
            "kind": "generic_mambalibs_runtime_surface_blocker",
            "family": manifest["family"],
            "path": manifest["path"],
            "issue": manifest["issue"],
            "status": manifest["support_status"],
            "owner_refs": ["#714"],
            "owned": True,
            "reason": "generic mambalibs runtime surface is declared but not fully pass-ready",
        }
        for manifest in selected
        if manifest["support_status"] in {"xfail", "blocker"}
    ]
    return {
        "families": [manifest["family"] for manifest in selected],
        "status_counts": dict(sorted(statuses.items())),
        "blockers": blockers,
    }


def build_report(show: int) -> dict[str, Any]:
    manifests, manifest_errors = fixture_manifests()
    by_family = manifest_by_family(manifests)
    registered, register_errors = expected_kits()
    by_library: dict[str, dict[str, Any]] = {}
    blockers: list[dict[str, Any]] = [*manifest_errors, *register_errors]
    status_counts: Counter[str] = Counter()

    for kit in NATIVE_KITS:
        item, item_blockers = kit_report(kit, by_family, registered)
        by_library[kit.id] = item
        status_counts[item["status"]] += 1
        blockers.extend(item_blockers)

    generic = generic_mode2_status(manifests)
    blockers.extend(generic["blockers"])
    support_status_counts = Counter(
        manifest["support_status"] or "not_statused"
        for manifest in manifests
    )
    import_gate_manifests = [
        manifest for manifest in manifests if str(manifest.get("family", "")).endswith("_import_gate")
    ]
    path_errors = []
    if not MAMBALIBS_README.exists():
        path_errors.append(
            {
                "kind": "mambalibs_readme_missing",
                "path": rel_path(MAMBALIBS_README),
                "owner_refs": ["#714"],
                "owned": True,
            }
        )
    blockers.extend(path_errors)

    unowned_gap_count = sum(1 for blocker in blockers if not blocker.get("owned"))
    ready = (
        not manifest_errors
        and not register_errors
        and not path_errors
        and unowned_gap_count == 0
        and status_counts.get("pass", 0) == len(NATIVE_KITS)
        and not generic["blockers"]
    )
    return {
        "schema_version": 1,
        "owner_issue": "#714",
        "ready": ready,
        "status": "green" if ready else "red",
        "readiness_classes": {
            "mambalibs_native_kit": {
                "separate_from_pure_python_packages": True,
                "separate_from_cpython_c_extension_abi": True,
                "counts_as_pure_python_package": False,
                "counts_as_cpython_extension_abi": False,
            },
            "pure_python_package_management": {
                "owned_by": "#711/#pkg-management",
                "counts_mambalibs_gaps": False,
            },
        },
        "registered_runtime_kits": sorted(registered),
        "generic_mode2_runtime_surface": generic,
        "counts": {
            "native_kits": len(NATIVE_KITS),
            "pass": status_counts["pass"],
            "fail": status_counts["fail"],
            "blocker": status_counts["blocker"],
            "fixture_manifests": len(manifests),
            "import_gate_manifests": len(import_gate_manifests),
            "support_status_pass": support_status_counts["pass"],
            "support_status_xfail": support_status_counts["xfail"],
            "support_status_blocker": support_status_counts["blocker"],
            "support_status_missing": support_status_counts["not_statused"],
            "registered_runtime_kits": len(registered),
            "manifest_errors": len(manifest_errors),
            "register_errors": len(register_errors),
            "path_errors": len(path_errors),
            "blockers": len(blockers),
            "unowned_gap_count": unowned_gap_count,
        },
        "by_library": by_library,
        "blockers": blockers[:show],
        "blocker_count": len(blockers),
        "evidence_commands": [
            "python3.12 apps/mamba/tests/harness/cpython/tools/mambalibs_readiness.py --json",
            "cargo build -p mamba",
            "rg \"mambalibs\" apps/mamba apps/mamba/mambalibs",
            "target/debug/mamba --help",
            "cargo test -p mamba --test mambalibs -- --nocapture",
            "cargo test -p mamba --test schema_gates mambalibs_readiness_gate_714 -- --nocapture",
        ],
    }


def print_human(report: dict[str, Any]) -> None:
    counts = report["counts"]
    print(f"mambalibs readiness: {report['status']}")
    print(
        "  native_kits={native_kits} pass={pass} fail={fail} blocker={blocker} "
        "fixtures={fixture_manifests} import_gates={import_gate_manifests}".format(
            **counts
        )
    )
    for kit, item in report["by_library"].items():
        print(
            f"- {item['status']}: {kit} {item['namespace']} "
            f"registered={item['registered_in_runtime']} import_gate={item['import_gate_status']}"
        )
    for blocker in report["blockers"][:5]:
        label = blocker.get("kit") or blocker.get("family") or blocker.get("path") or blocker.get("kind")
        print(f"  blocker: {blocker['kind']} {label}")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--show", type=int, default=20)
    args = parser.parse_args(argv)

    report = build_report(args.show)
    if args.json:
        print(json.dumps(report, sort_keys=True))
    else:
        print_human(report)
    return 0 if report["ready"] else EXIT_NOT_READY


if __name__ == "__main__":
    raise SystemExit(main())
