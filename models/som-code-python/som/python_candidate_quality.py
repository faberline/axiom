"""Fail-closed quality checks for staged Python candidate fixture packs.

This module reads source text and metadata only.  It never imports a candidate,
runs an oracle, or reads a SOM corpus, model, or training run.
"""

from __future__ import annotations

import argparse
import ast
import hashlib
import json
from collections import Counter
from collections.abc import Iterable, Mapping
from dataclasses import dataclass
from pathlib import Path
from typing import Any


FAILURE_MODES = frozenset(
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
"""The only allowed near-miss mechanisms in the first staged Python packs."""

_FORBIDDEN_ROOT_PARTS = frozenset({"families", "smoke", "runs", "models", "weights"})
_MEANINGFUL_NODES = (ast.If, ast.For, ast.AsyncFor, ast.While, ast.Try, ast.With, ast.AsyncWith, ast.Call, ast.Await)
_TRIVIAL_NODES = (ast.Constant, ast.List, ast.Tuple, ast.Dict, ast.Set)
MIN_AST_OVERLAP = 0.30


@dataclass(frozen=True)
class CandidateSource:
    candidate_id: str
    source: str
    source_hash: str
    normalized_ast: str
    node_counts: Counter[str]


def _failure(code: str, message: str, candidate_id: str | None = None) -> dict[str, str]:
    item = {"code": code, "message": message}
    if candidate_id is not None:
        item["candidate_id"] = candidate_id
    return item


def _normalise_ast(source: str) -> tuple[str, Counter[str]]:
    """Remove names, literals, comments, and whitespace while retaining syntax."""
    tree = ast.parse(source)
    counts: Counter[str] = Counter(type(node).__name__ for node in ast.walk(tree))

    # ``ast.dump`` excludes comments, line locations, and whitespace.  Keep
    # identifiers and literals: changing either can be a real near-miss.
    return ast.dump(tree, annotate_fields=True, include_attributes=False), counts


def _ast_overlap(left: Counter[str], right: Counter[str]) -> float:
    names = set(left) | set(right)
    if not names:
        return 1.0
    shared = sum(min(left[name], right[name]) for name in names)
    total = sum(max(left[name], right[name]) for name in names)
    return shared / total if total else 1.0


def _has_meaningful_control_or_calls(source: str) -> bool:
    return any(isinstance(node, _MEANINGFUL_NODES) for node in ast.walk(ast.parse(source)))


def _is_pure_trivial_return(source: str) -> bool:
    tree = ast.parse(source)
    functions = [node for node in tree.body if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef))]
    if not functions:
        return False
    for function in functions:
        if len(function.body) != 1 or not isinstance(function.body[0], ast.Return):
            return False
        value = function.body[0].value
        if not isinstance(value, _TRIVIAL_NODES):
            return False
    return True


def _outer_returns(function: ast.FunctionDef | ast.AsyncFunctionDef) -> list[ast.Return]:
    """Return only returns owned by ``function``, not a nested scope."""
    returns: list[ast.Return] = []

    def visit(node: ast.AST) -> None:
        if node is not function and isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef, ast.Lambda, ast.ClassDef)):
            return
        if isinstance(node, ast.Return):
            returns.append(node)
            return
        for child in ast.iter_child_nodes(node):
            visit(child)

    visit(function)
    return returns


def _names_loaded_outside_nested_scopes(expression: ast.AST | None) -> set[str]:
    """Collect names used by one expression without treating lambda locals as inputs."""
    if expression is None:
        return set()
    names: set[str] = set()

    def visit(node: ast.AST) -> None:
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef, ast.Lambda, ast.ClassDef)):
            return
        if isinstance(node, ast.Name) and isinstance(node.ctx, ast.Load):
            names.add(node.id)
        for child in ast.iter_child_nodes(node):
            visit(child)

    visit(expression)
    return names


def _assignment_targets(node: ast.AST) -> set[str]:
    targets: list[ast.AST]
    if isinstance(node, (ast.Assign, ast.AnnAssign, ast.NamedExpr)):
        targets = list(node.targets) if isinstance(node, ast.Assign) else [node.target]
    else:
        return set()
    return {name.id for target in targets for name in ast.walk(target) if isinstance(name, ast.Name) and isinstance(name.ctx, ast.Store)}


def _assignment_value(node: ast.AST) -> ast.AST | None:
    if isinstance(node, (ast.Assign, ast.AnnAssign, ast.NamedExpr)):
        return node.value
    return None


def _setup_derived_names(function: ast.FunctionDef | ast.AsyncFunctionDef) -> set[str]:
    """Find local values built by calls, then values derived from those values.

    This is intentionally narrow.  It does not try to prove general program
    behaviour.  It only follows local assignment chains in the public
    entrypoint, which makes it safe to use as a fail-closed fixture heuristic.
    """
    assignments: list[tuple[set[str], ast.AST]] = []

    def visit(node: ast.AST) -> None:
        if node is not function and isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef, ast.Lambda, ast.ClassDef)):
            return
        value = _assignment_value(node)
        targets = _assignment_targets(node)
        if value is not None and targets:
            assignments.append((targets, value))
        for child in ast.iter_child_nodes(node):
            visit(child)

    visit(function)

    derived: set[str] = set()
    changed = True
    while changed:
        changed = False
        for targets, value in assignments:
            value_names = _names_loaded_outside_nested_scopes(value)
            creates_value = any(isinstance(item, (ast.Call, ast.Await)) for item in ast.walk(value))
            if creates_value or value_names & derived:
                before = len(derived)
                derived.update(targets)
                changed = changed or len(derived) != before
    return derived


def _solve_function(source: str) -> ast.FunctionDef | ast.AsyncFunctionDef | None:
    tree = ast.parse(source)
    return next((node for node in tree.body if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)) and node.name == "solve"), None)


def _solve_return_uses_setup(source: str) -> bool:
    function = _solve_function(source)
    if function is None:
        return False
    derived = _setup_derived_names(function)
    returns = _outer_returns(function)
    return bool(derived and returns and any(_names_loaded_outside_nested_scopes(item.value) & derived for item in returns))


def _dead_setup_return_failure(gold_source: str, candidate_source: str, candidate_id: str) -> dict[str, str] | None:
    """Reject a fake patch that builds setup but discards all of its results.

    The gold guard avoids rejecting a deliberately literal task.  The check is
    only a local dataflow rule: a candidate must create setup-derived values,
    return at least once, and no return may use one of those values.
    """
    if not _solve_return_uses_setup(gold_source):
        return None
    function = _solve_function(candidate_source)
    if function is None:
        return None
    derived = _setup_derived_names(function)
    returns = _outer_returns(function)
    if derived and returns and not any(_names_loaded_outside_nested_scopes(item.value) & derived for item in returns):
        return _failure(
            "dead_setup_return",
            "solve builds setup-derived values but every return ignores them while gold returns a setup-derived value",
            candidate_id,
        )
    return None


def _entrypoint_failures(source: str, candidate_id: str) -> list[dict[str, str]]:
    """Reject duplicate top-level functions and silent implementation overrides.

    A candidate must describe one implementation per public entrypoint.  A
    later ``solve`` (or any other function) can otherwise hide a meaningful
    earlier implementation while making the source look structurally rich.
    This check only inspects the AST; it never imports or executes the source.
    """
    tree = ast.parse(source)
    functions = [node for node in tree.body if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef))]
    by_name: dict[str, list[ast.FunctionDef | ast.AsyncFunctionDef]] = {}
    for function in functions:
        by_name.setdefault(function.name, []).append(function)

    failures: list[dict[str, str]] = []
    for name, definitions in by_name.items():
        if len(definitions) < 2:
            continue
        failures.append(_failure("duplicate_top_level_entrypoint", f"top-level function {name!r} is defined {len(definitions)} times", candidate_id))
        earlier, final = definitions[-2], definitions[-1]
        earlier_source = ast.get_source_segment(source, earlier) or ""
        final_source = ast.get_source_segment(source, final) or ""
        if _has_meaningful_control_or_calls(earlier_source) and _is_pure_trivial_return(final_source):
            failures.append(_failure("trivial_entrypoint_override", f"later {name!r} definition replaces an earlier meaningful implementation with a literal return", candidate_id))
    return failures


def _safe_source(root: Path, module: str) -> tuple[str | None, list[dict[str, str]]]:
    root = root.resolve()
    path = (root / module).resolve()
    try:
        path.relative_to(root)
    except ValueError:
        return None, [_failure("candidate_path_escape", "candidate module escapes its family directory")]
    if not path.is_file():
        return None, [_failure("candidate_source_missing", f"candidate source is missing: {module}")]
    try:
        return path.read_text(encoding="utf-8"), []
    except UnicodeDecodeError:
        return None, [_failure("candidate_source_encoding", f"candidate source is not UTF-8: {module}")]


def audit_family(path: Path) -> dict[str, Any]:
    """Audit one staged ``family.json`` without executing any fixture code."""
    failures: list[dict[str, str]] = []
    try:
        metadata = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        return {"family_path": str(path), "status": "failed", "failures": [_failure("family_metadata_invalid", str(exc))], "candidates": []}
    if not isinstance(metadata, Mapping):
        return {"family_path": str(path), "status": "failed", "failures": [_failure("family_metadata_invalid", "family metadata must be an object")], "candidates": []}
    candidates = metadata.get("candidates")
    if not isinstance(candidates, list):
        return {"family_id": metadata.get("family_id"), "family_path": str(path), "status": "failed", "failures": [_failure("candidates_missing", "metadata must contain a candidates list")], "candidates": []}

    gold_entries = [entry for entry in candidates if isinstance(entry, Mapping) and entry.get("id") == "gold"]
    misses = [entry for entry in candidates if isinstance(entry, Mapping) and str(entry.get("id", "")).startswith("miss_")]
    if len(gold_entries) != 1:
        failures.append(_failure("gold_count", f"expected exactly one gold candidate; found {len(gold_entries)}"))
    if len(misses) != 6 or len(candidates) != 7:
        failures.append(_failure("near_miss_count", f"expected one gold and six near misses; found {len(misses)} near misses in {len(candidates)} candidates"))

    parsed: dict[str, CandidateSource] = {}
    candidate_reports: list[dict[str, Any]] = []
    declared_hashes = metadata.get("source_hashes")
    if not isinstance(declared_hashes, Mapping):
        failures.append(_failure("source_hashes_missing", "metadata must declare source_hashes"))
        declared_hashes = {}

    for entry in candidates:
        if not isinstance(entry, Mapping):
            failures.append(_failure("candidate_metadata_invalid", "candidate entry must be an object"))
            continue
        candidate_id = entry.get("id")
        module = entry.get("module")
        entry_failures: list[dict[str, str]] = []
        if not isinstance(candidate_id, str) or not candidate_id:
            entry_failures.append(_failure("candidate_id_invalid", "candidate id must be a non-empty string"))
            candidate_id = "<invalid>"
        if not isinstance(module, str) or not module:
            entry_failures.append(_failure("candidate_module_invalid", "candidate module must be a non-empty path", candidate_id))
            candidate_reports.append({"candidate_id": candidate_id, "failures": entry_failures})
            failures.extend(entry_failures)
            continue
        source, read_failures = _safe_source(path.parent, module)
        entry_failures.extend(read_failures)
        if source is not None:
            source_hash = hashlib.sha256(source.encode("utf-8")).hexdigest()
            expected = declared_hashes.get(module)
            if expected != source_hash:
                entry_failures.append(_failure("source_hash_mismatch", f"source hash for {module} does not match metadata", candidate_id))
            try:
                normalised, nodes = _normalise_ast(source)
                parsed[candidate_id] = CandidateSource(candidate_id, source, source_hash, normalised, nodes)
                entry_failures.extend(_entrypoint_failures(source, candidate_id))
            except SyntaxError as exc:
                entry_failures.append(_failure("candidate_syntax_invalid", str(exc), candidate_id))
        candidate_reports.append({"candidate_id": candidate_id, "module": module, "failures": entry_failures})
        failures.extend(entry_failures)

    source_hashes = [item.source_hash for item in parsed.values()]
    if len(source_hashes) != len(set(source_hashes)):
        failures.append(_failure("candidate_source_duplicate", "candidate source byte hashes must be distinct"))
    structures: dict[str, str] = {}
    for item in parsed.values():
        previous = structures.get(item.normalized_ast)
        if previous is not None:
            failures.append(_failure("comment_or_whitespace_variant", f"{item.candidate_id} has the same normalised AST as {previous}", item.candidate_id))
        else:
            structures[item.normalized_ast] = item.candidate_id

    failure_modes: list[str] = []
    for miss in misses:
        mode = miss.get("failure_mode") if isinstance(miss, Mapping) else None
        candidate_id = str(miss.get("id", "<invalid>")) if isinstance(miss, Mapping) else "<invalid>"
        if not isinstance(mode, str):
            failures.append(_failure("failure_mode_missing", "each near miss must declare failure_mode", candidate_id))
        elif mode not in FAILURE_MODES:
            failures.append(_failure("failure_mode_unknown", f"failure_mode {mode!r} is outside the fixed taxonomy", candidate_id))
        else:
            failure_modes.append(mode)
    if len(failure_modes) != len(set(failure_modes)):
        failures.append(_failure("failure_mode_duplicate", "the six near misses must use distinct failure modes"))

    gold = parsed.get("gold")
    if gold is not None:
        gold_meaningful = _has_meaningful_control_or_calls(gold.source)
        for miss in misses:
            candidate_id = miss.get("id") if isinstance(miss, Mapping) else None
            item = parsed.get(candidate_id) if isinstance(candidate_id, str) else None
            if item is None:
                continue
            overlap = _ast_overlap(gold.node_counts, item.node_counts)
            report = next((candidate for candidate in candidate_reports if candidate["candidate_id"] == candidate_id), None)
            if report is not None:
                report["ast_overlap_with_gold"] = overlap
            if overlap < MIN_AST_OVERLAP:
                failures.append(_failure("ast_overlap_too_low", f"normalised AST overlap {overlap:.3f} is below {MIN_AST_OVERLAP:.2f}", candidate_id))
            if gold_meaningful and _is_pure_trivial_return(item.source):
                failures.append(_failure("trivial_literal_return", "near miss is only a literal return while gold has meaningful control flow or calls", candidate_id))
            dead_setup = _dead_setup_return_failure(gold.source, item.source, candidate_id)
            if dead_setup is not None:
                failures.append(dead_setup)

    return {
        "family_id": metadata.get("family_id"),
        "family_path": str(path),
        "status": "passed" if not failures else "failed",
        "failures": failures,
        "candidates": candidate_reports,
    }


def audit_staged_candidate_quality(root: str | Path) -> dict[str, Any]:
    """Return a fail-closed report for a supplied staged fixture-pack root."""
    root_path = Path(root).resolve()
    if not root_path.is_dir():
        raise ValueError(f"staged fixture root does not exist: {root_path}")
    if _FORBIDDEN_ROOT_PARTS & {part.lower() for part in root_path.parts}:
        raise ValueError("candidate quality audit only accepts a staged fixture-pack root")
    family_paths = sorted(root_path.rglob("family.json"))
    if not family_paths:
        raise ValueError(f"no staged family.json files found under {root_path}")
    families = [audit_family(path) for path in family_paths]
    failures = [failure for family in families for failure in family["failures"]]
    return {
        "audit": "som-python-staged-candidate-quality-v1",
        "root": str(root_path),
        "status": "passed" if not failures else "failed",
        "families": families,
        "summary": {"family_count": len(families), "failed_family_count": sum(family["status"] == "failed" for family in families), "failure_count": len(failures)},
    }


def render_markdown(report: Mapping[str, Any]) -> str:
    lines = ["# Python staged candidate quality audit", "", f"Status: **{report['status']}**", "", "| Item | Count |", "| --- | ---: |"]
    for key, value in report["summary"].items():
        lines.append(f"| {key} | {value} |")
    for family in report["families"]:
        lines.extend(["", f"## {family.get('family_id') or family['family_path']}", ""])
        if not family["failures"]:
            lines.append("Passed.")
        else:
            for failure in family["failures"]:
                candidate = f" ({failure['candidate_id']})" if "candidate_id" in failure else ""
                lines.append(f"- `{failure['code']}`{candidate}: {failure['message']}")
    return "\n".join(lines) + "\n"


def write_report(report: Mapping[str, Any], output_dir: str | Path) -> tuple[Path, Path]:
    directory = Path(output_dir).resolve()
    directory.mkdir(parents=True, exist_ok=True)
    json_path = directory / "candidate-quality-audit.json"
    markdown_path = directory / "candidate-quality-audit.md"
    json_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    markdown_path.write_text(render_markdown(report), encoding="utf-8")
    return json_path, markdown_path


def main(argv: Iterable[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Audit staged Python fixture candidate quality without executing fixtures.")
    parser.add_argument("root", type=Path, help="staged fixture-pack root")
    parser.add_argument("--output", required=True, type=Path, help="directory for JSON and Markdown reports")
    args = parser.parse_args(argv)
    report = audit_staged_candidate_quality(args.root)
    write_report(report, args.output)
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
