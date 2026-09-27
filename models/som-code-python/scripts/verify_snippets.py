#!/usr/bin/env python3
"""Verify the snippet ISA against the corpus that uses it.

The ISA is ``data/snippets/<library>/<id>.json``; each file is one snippet
``{id, description, imports, template}``. A snippet earns its place by being
used, so the check reads each family's stored ``decompiled.ops`` (which
``scripts/verify_curation.py`` keeps equal to a fresh decompile) rather than
trusting the template alone.

Rules this script owns, one defect line per violation:

- Schema: exactly the four keys; ``id`` equals the file name; a non-empty
  ``description`` and ``template``; every ``imports`` entry parses as one
  import statement.
- Syntax: tags are ``{{name}}`` or a one-level ``{{#name}}...{{/name}}``
  section (``models/som-core/docs/reference/layer-records.md``).
- No holes: every template line that carries a placeholder still has a
  non-whitespace literal once its tags are removed, so ``    {{body}}``
  cannot smuggle arbitrary code into a snippet.
- At least two families: a snippet ``INSERT_SNIPPET`` uses in fewer than
  :data:`MIN_FAMILIES` distinct families is a one-off, and belongs in the
  corpus as a literal block, not in the ISA. An operation naming an id the
  ISA lacks is a defect too.
- Renders to Python: the template, filled with its first use's params,
  parses with ``ast.parse``.

It then prints each snippet's family count and the corpus coverage,
``snippet_blocks / all blocks``. Exit 0 with no defects, 1 otherwise.
"""

from __future__ import annotations

import argparse
import ast
import json
import sys
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent))
from decompile_gold import TAG, DecompileError, _parse, _render  # noqa: E402

DATA = Path(__file__).resolve().parent.parent / "data"
MIN_FAMILIES = 2
KEYS = {"id", "description", "imports", "template"}


def schema_defects(path: Path, snippet: Any) -> list[str]:
    if not isinstance(snippet, dict) or set(snippet) != KEYS:
        got = sorted(snippet) if isinstance(snippet, dict) else type(snippet).__name__
        return [f"{path}: fields must be {sorted(KEYS)}; got {got}"]
    out = []
    if snippet["id"] != path.stem:
        out.append(f"{path}: id {snippet['id']!r} does not match the file name {path.stem!r}")
    for key in ("description", "template"):
        if not isinstance(snippet[key], str) or not snippet[key].strip():
            out.append(f"{path}: {key} must be a non-empty string")
    imports = snippet["imports"]
    if not isinstance(imports, list):
        out.append(f"{path}: imports must be a list")
        return out
    for stmt in imports:
        try:
            tree = ast.parse(stmt) if isinstance(stmt, str) else None
        except SyntaxError:
            tree = None
        if tree is None or len(tree.body) != 1 or not isinstance(tree.body[0], (ast.Import, ast.ImportFrom)):
            out.append(f"{path}: import {stmt!r} is not one import statement")
    return out


def hole_defects(path: Path, template: str) -> list[str]:
    out = []
    for number, line in enumerate(template.split("\n"), 1):
        has_var = any(not hit.group(1) for hit in TAG.finditer(line))
        if has_var and not TAG.sub("", line).strip():
            out.append(f"{path}: template line {number} {line!r} is only a placeholder")
    return out


def uses_by_id(corpus: Path) -> tuple[dict[str, list[tuple[str, dict]]], int, int]:
    """Every ``INSERT_SNIPPET`` in the corpus as ``id -> [(family, params)]``, plus block counts."""
    uses: dict[str, list[tuple[str, dict]]] = {}
    snippet_blocks = total = 0
    for path in sorted((corpus / "families").glob("*/family.json")):
        ops = json.loads(path.read_text(encoding="utf-8")).get("decompiled", {}).get("ops", [])
        for op in ops:
            if op["op"] in ("INSERT_SNIPPET", "INSERT_BLOCK"):
                total += 1
            if op["op"] == "INSERT_SNIPPET":
                snippet_blocks += 1
                uses.setdefault(op["id"], []).append((path.parent.name, op["params"]))
    return uses, snippet_blocks, total


def verify(snippets_dir: Path, corpus: Path) -> tuple[list[str], list[str]]:
    """Return ``(defects, report lines)``."""
    uses, snippet_blocks, total = uses_by_id(corpus)
    defects: list[str] = []
    report: list[str] = []
    seen: set[str] = set()
    for source in sorted(snippets_dir.glob("*/*.json")):
        path = source.relative_to(snippets_dir)
        try:
            snippet = json.loads(source.read_text(encoding="utf-8"))
        except json.JSONDecodeError as exc:
            defects.append(f"{path}: not valid JSON ({exc})")
            continue
        found = schema_defects(path, snippet)
        defects += found
        if found:
            continue
        seen.add(snippet["id"])
        try:
            parts = _parse(snippet["template"])
        except DecompileError as exc:
            defects.append(f"{path}: {exc}")
            continue
        defects += hole_defects(path, snippet["template"])
        families = sorted({family for family, _ in uses.get(snippet["id"], [])})
        report.append(f"  {snippet['id']:32} {len(families):3} families  {len(uses.get(snippet['id'], [])):3} blocks")
        if len(families) < MIN_FAMILIES:
            defects.append(
                f"{path}: used by {len(families)} families {families}; "
                f"a snippet needs at least {MIN_FAMILIES}"
            )
            continue
        try:
            ast.parse(_render(parts, uses[snippet["id"]][0][1]))
        except (SyntaxError, KeyError, TypeError) as exc:
            defects.append(f"{path}: first use in {families[0]} does not render to Python ({exc})")
    for snippet_id in sorted(uses.keys() - seen):
        defects.append(f"{corpus}: ops use snippet {snippet_id!r}, which the ISA does not define")
    report.append(f"  coverage: {snippet_blocks} / {total} blocks are INSERT_SNIPPET")
    return defects, report


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--snippets", type=Path, default=DATA / "snippets", help="ISA directory (default data/snippets)")
    ap.add_argument("--corpus", type=Path, default=DATA / "curated", help="corpus layer whose ops are counted (default data/curated)")
    args = ap.parse_args()
    defects, report = verify(args.snippets, args.corpus)
    print("\n".join(report))
    for defect in defects:
        print(f"DEFECT {defect}")
    if defects:
        print(f"[RESULT: FAILURE] {len(defects)} snippet defects")
        return 1
    print("[RESULT: SUCCESS] 0 snippet defects")
    return 0


if __name__ == "__main__":
    sys.exit(main())
