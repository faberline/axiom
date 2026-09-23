#!/usr/bin/env python3
"""Decompile each family's gold candidate into the facts its caption must agree with.

A caption is prose a person writes about the gold candidate. ``decompiled``
is what the gold source says about itself, measured from its AST so the
two cannot drift apart:

- ``surface``: the top-level components in source order. A class carries its
  bases and method names; a function carries its parameter names and, when a
  decorator registers it, the HTTP route (``GET /items``) or ``command`` for
  a CLI entry point; a module-level assignment carries its name.
- ``imports``: the third-party top-level modules the gold imports. The
  standard library is left out because it is not a dependency decision.
- ``raises``: the exception classes the gold raises by name.
- ``status_codes``: every ``status_code`` the gold declares, on a route
  decorator or an ``HTTPException``.

``--write`` stores the block under ``decompiled`` in each ``family.json``.
``scripts/verify_curation.py`` re-measures it on every run and fails on a
difference, the discipline ``caught_by`` already follows. ``--draft <dir>``
writes one Markdown sheet per family, with the requirement, the measured
facts, the gold source, and the fixture's tests with their assertions, for
the person writing the caption. Without a flag it prints the measured block
for every family and changes nothing.
"""

from __future__ import annotations

import argparse
import ast
import json
import re
import sys
from pathlib import Path

DATA = Path(__file__).resolve().parent.parent / "data"
CORPUS = DATA / "curated"
MATERIALS = CORPUS / "families"
FIXTURES = CORPUS / "fixtures"


def use_corpus(corpus: Path) -> None:
    """Point the module at another corpus layer, e.g. ``data/user``."""
    global CORPUS, MATERIALS, FIXTURES
    CORPUS = corpus.resolve()
    MATERIALS = CORPUS / "families"
    FIXTURES = CORPUS / "fixtures"
HTTP_METHODS = {"get", "post", "put", "patch", "delete", "head", "options", "websocket"}
CLI_DECORATORS = {"command", "callback"}
CLASS_NAME_RE = re.compile(r"[A-Z][A-Za-z0-9]*")
HTTP_CONST_RE = re.compile(r"HTTP_(\d{3})_")
KEY_ORDER = [
    "family_id", "domain", "area", "capability", "requirement", "skeleton",
    "caption", "rationale", "oracle", "decompiled", "candidates",
]


def _decorator_name(dec: ast.expr) -> str | None:
    node = dec.func if isinstance(dec, ast.Call) else dec
    if isinstance(node, ast.Attribute):
        return node.attr
    if isinstance(node, ast.Name):
        return node.id
    return None


def _route(fn: ast.FunctionDef | ast.AsyncFunctionDef) -> str | None:
    for dec in fn.decorator_list:
        name = _decorator_name(dec)
        if name in HTTP_METHODS and isinstance(dec, ast.Call) and dec.args:
            first = dec.args[0]
            if isinstance(first, ast.Constant) and isinstance(first.value, str):
                return f"{name.upper()} {first.value}"
        if name in CLI_DECORATORS:
            return "command"
    return None


def _params(fn: ast.FunctionDef | ast.AsyncFunctionDef) -> list[str]:
    a = fn.args
    names = [p.arg for p in (*a.posonlyargs, *a.args, *a.kwonlyargs)]
    if a.vararg:
        names.append("*" + a.vararg.arg)
    if a.kwarg:
        names.append("**" + a.kwarg.arg)
    return names


def _assigned_names(node: ast.Assign | ast.AnnAssign) -> list[str]:
    targets = node.targets if isinstance(node, ast.Assign) else [node.target]
    out: list[str] = []
    for t in targets:
        for n in ast.walk(t):
            if isinstance(n, ast.Name):
                out.append(n.id)
    return out


def measure(source: str) -> dict:
    """Return the ``decompiled`` block for one candidate source."""
    tree = ast.parse(source)
    surface: list[dict] = []
    for node in tree.body:
        if isinstance(node, ast.ClassDef):
            surface.append({
                "kind": "class",
                "name": node.name,
                "bases": [ast.unparse(b) for b in node.bases],
                "methods": [
                    m.name for m in node.body
                    if isinstance(m, (ast.FunctionDef, ast.AsyncFunctionDef))
                ],
            })
        elif isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
            entry = {
                "kind": "function" if isinstance(node, ast.FunctionDef) else "async_function",
                "name": node.name,
                "params": _params(node),
            }
            route = _route(node)
            if route:
                entry["route"] = route
            surface.append(entry)
        elif isinstance(node, (ast.Assign, ast.AnnAssign)):
            for name in _assigned_names(node):
                surface.append({"kind": "assign", "name": name})

    imports: set[str] = set()
    for node in ast.walk(tree):
        if isinstance(node, ast.Import):
            for alias in node.names:
                imports.add(alias.name.split(".")[0])
        elif isinstance(node, ast.ImportFrom) and node.module and node.level == 0:
            imports.add(node.module.split(".")[0])
    third_party = sorted(m for m in imports if m not in sys.stdlib_module_names)

    raises: set[str] = set()
    codes: set[int] = set()
    for node in ast.walk(tree):
        if isinstance(node, ast.Raise) and node.exc is not None:
            exc = node.exc
            callee = exc.func if isinstance(exc, ast.Call) else exc
            name = None
            if isinstance(callee, ast.Name):
                name = callee.id
            elif isinstance(callee, ast.Attribute):
                name = callee.attr
            if name and CLASS_NAME_RE.fullmatch(name):
                raises.add(name)
        if isinstance(node, ast.Call):
            callee = node.func
            cname = callee.id if isinstance(callee, ast.Name) else (
                callee.attr if isinstance(callee, ast.Attribute) else None
            )
            if cname == "HTTPException" and node.args:
                first = node.args[0]
                if isinstance(first, ast.Constant) and isinstance(first.value, int):
                    codes.add(first.value)
        if isinstance(node, ast.keyword) and node.arg == "status_code":
            value = node.value
            if isinstance(value, ast.Constant) and isinstance(value.value, int):
                codes.add(value.value)
            elif isinstance(value, ast.Attribute):
                hit = HTTP_CONST_RE.match(value.attr)
                if hit:
                    codes.add(int(hit.group(1)))

    return {
        "surface": surface,
        "imports": third_party,
        "raises": sorted(raises),
        "status_codes": sorted(codes),
    }


def families() -> list[Path]:
    return sorted(d for d in MATERIALS.iterdir() if (d / "family.json").is_file())


def gold_source(fam: Path) -> str:
    meta = json.loads((fam / "family.json").read_text(encoding="utf-8"))
    gold = next(c for c in meta["candidates"] if c["kind"] == "gold")
    return (fam / gold["module"]).read_text(encoding="utf-8")


def ordered(meta: dict) -> dict:
    out = {k: meta[k] for k in KEY_ORDER if k in meta}
    out.update({k: v for k, v in meta.items() if k not in out})
    return out


def write_block(fam: Path, block: dict) -> bool:
    path = fam / "family.json"
    meta = json.loads(path.read_text(encoding="utf-8"))
    if meta.get("decompiled") == block:
        return False
    meta["decompiled"] = block
    path.write_text(json.dumps(ordered(meta), indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    return True


def fixture_for(fam: Path) -> Path | None:
    hits = sorted(FIXTURES.glob(f"test_{fam.name.split('-')[0]}_*.py"))
    return hits[0] if hits else None


def test_sheet(fixture: Path) -> list[str]:
    tree = ast.parse(fixture.read_text(encoding="utf-8"))
    lines: list[str] = []
    for node in ast.walk(tree):
        if not isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)) or not node.name.startswith("test"):
            continue
        doc = ast.get_docstring(node)
        lines.append(f"- `{node.name}`" + (f" — {doc.strip().splitlines()[0]}" if doc else ""))
        for inner in ast.walk(node):
            if isinstance(inner, ast.Assert):
                lines.append(f"    - assert {ast.unparse(inner.test)[:140]}")
            elif isinstance(inner, ast.With):
                for item in inner.items:
                    text = ast.unparse(item.context_expr)
                    if "raises" in text:
                        lines.append(f"    - with {text[:140]}")
    return lines


def draft(fam: Path, block: dict, out_dir: Path) -> None:
    meta = json.loads((fam / "family.json").read_text(encoding="utf-8"))
    fixture = fixture_for(fam)
    parts = [
        f"# {fam.name}",
        "",
        f"requirement: {meta['requirement']}",
        f"skeleton: {meta['skeleton']}",
        f"teaches: {meta['rationale']['teaches']}",
        f"why: {meta['rationale']['why']}",
        f"oracle: {meta.get('oracle')}",
        "",
        "## decompiled",
        "```json",
        json.dumps(block, indent=1),
        "```",
        "",
        "## near misses",
    ]
    for cand in meta["candidates"]:
        if cand["kind"] != "gold":
            parts.append(f"- {cand['id']} [{cand['failure_mode']}] {cand['why_wrong']} caught_by={cand.get('caught_by')}")
    parts += ["", "## gold source", "```python", gold_source(fam).rstrip(), "```", ""]
    parts.append("## fixture tests" if fixture else "## fixture tests: none")
    if fixture:
        parts.extend(test_sheet(fixture))
    out_dir.mkdir(parents=True, exist_ok=True)
    (out_dir / f"{fam.name}.md").write_text("\n".join(parts) + "\n", encoding="utf-8")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--write", action="store_true", help="store the block under decompiled in each family.json")
    ap.add_argument("--draft", type=Path, help="write one caption-authoring sheet per family into this directory")
    ap.add_argument("--corpus", type=Path, default=CORPUS, help="corpus layer to read (default data/curated; data/user for your own families)")
    ap.add_argument("families", nargs="*", help="family number prefixes to limit to, e.g. 01 48")
    args = ap.parse_args()
    use_corpus(args.corpus)

    selected = families()
    if args.families:
        selected = [f for f in selected if f.name.split("-")[0] in set(args.families)]
    if not selected:
        print(f"no families under {MATERIALS}")
        return 1
    changed = 0
    for fam in selected:
        block = measure(gold_source(fam))
        if args.write:
            changed += write_block(fam, block)
        if args.draft:
            draft(fam, block, args.draft)
        if not args.write and not args.draft:
            print(json.dumps({"family": fam.name, **block}))
    print(f"families: {len(selected)}  written: {changed}" if args.write else f"families: {len(selected)}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
