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
- ``topology``, ``ops``, ``coverage``: the L2 topology and L3 operation list
  that rebuild the gold, and how many of its blocks the snippet ISA
  expresses (``models/som-core/docs/reference/layer-records.md``). A block
  runs from the first non-blank line after the previous statement to its
  own last line, so a comment above a block travels with it; a block is
  ``INSERT_SNIPPET`` only when a snippet template matches its source in
  full, sections included, and the extracted params render back to the same
  bytes; otherwise ``INSERT_BLOCK``. ``scripts/verify_roundtrip.py`` proves
  the list rebuilds the gold.

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
SNIPPETS = DATA / "snippets"
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
    "caption", "plan", "rationale", "oracle", "decompiled", "candidates",
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
        **decompile({GOLD_PATH: source}),
    }


GOLD_PATH = "candidate.py"
TAG = re.compile(r"\{\{([#/]?)([a-zA-Z0-9_]+)\}\}")


class DecompileError(ValueError):
    """A program the layer records cannot express."""


def _parse(template: str) -> list[tuple[str, object]]:
    """Split a template into text, ``var`` and one-level ``section`` parts."""
    parts: list[tuple[str, object]] = []
    section: tuple[str, list] | None = None
    pos = 0
    for hit in TAG.finditer(template):
        target = section[1] if section else parts
        if hit.start() > pos:
            target.append(("text", template[pos:hit.start()]))
        sigil, name = hit.group(1), hit.group(2)
        if sigil == "#":
            if section:
                raise DecompileError(f"section {name!r} is nested")
            section = (name, [])
        elif sigil == "/":
            if not section or section[0] != name:
                raise DecompileError(f"section close {name!r} has no matching open")
            parts.append(("section", section))
            section = None
        else:
            target.append(("var", name))
        pos = hit.end()
    if section:
        raise DecompileError(f"section {section[0]!r} is never closed")
    if pos < len(template):
        parts.append(("text", template[pos:]))
    return parts


def _regex(parts: list[tuple[str, object]], named: bool) -> str:
    seen: set[str] = set()
    out = ""
    for kind, value in parts:
        if kind == "text":
            out += re.escape(value)
        elif kind == "var":
            if not named:
                out += "[^\\n]*"
            elif value in seen:
                out += f"(?P={value})"
            else:
                out += f"(?P<{value}>[^\\n]*)"
            seen.add(value)
        else:
            name, body = value
            out += f"(?P<{name}>(?:{_regex(body, named=False)})*)"
    return out


def _render(parts: list[tuple[str, object]], params: dict) -> str:
    out = ""
    for kind, value in parts:
        if kind == "text":
            out += value
        elif kind == "var":
            out += params[value]
        else:
            name, body = value
            out += "".join(_render(body, item) for item in params[name])
    return out


class Snippet:
    """One ISA template compiled for whole-block matching.

    A section becomes one outer group capturing every item; the items are
    then split off one at a time with the body's own regex. A match counts
    only when the extracted params render back to the block byte for byte,
    so an ambiguous split is a miss, never a wrong op.
    """

    def __init__(self, snippet_id: str, template: str) -> None:
        self.id = snippet_id
        self.parts = _parse(template)
        self.outer = re.compile(_regex(self.parts, named=True))
        self.items = {
            value[0]: re.compile(_regex(value[1], named=True))
            for kind, value in self.parts if kind == "section"
        }

    def match(self, text: str) -> dict | None:
        hit = self.outer.fullmatch(text)
        if not hit:
            return None
        params: dict = {}
        for kind, value in self.parts:
            if kind == "var":
                params[value] = hit.group(value)
            elif kind == "section":
                name = value[0]
                captured, pos, items = hit.group(name), 0, []
                while pos < len(captured):
                    item = self.items[name].match(captured, pos)
                    if not item or item.end() == pos:
                        return None
                    items.append(item.groupdict())
                    pos = item.end()
                params[name] = items
        return params if _render(self.parts, params) == text else None


def _snippet_patterns() -> list[Snippet]:
    patterns = []
    for path in sorted(SNIPPETS.glob("*/*.json")):
        snippet = json.loads(path.read_text(encoding="utf-8"))
        patterns.append(Snippet(snippet["id"], snippet["template"]))
    return patterns


def _block_kind(node: ast.stmt) -> str:
    if isinstance(node, ast.ClassDef):
        return "class"
    if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
        route = _route(node)
        if route == "command":
            return "command"
        return "route" if route else "function"
    if isinstance(node, (ast.Assign, ast.AnnAssign, ast.AugAssign)):
        return "constant"
    return "statement"


def _block_base_name(node: ast.stmt) -> str:
    if isinstance(node, (ast.ClassDef, ast.FunctionDef, ast.AsyncFunctionDef)):
        return node.name
    if isinstance(node, (ast.Assign, ast.AnnAssign, ast.AugAssign)):
        targets = node.targets if isinstance(node, ast.Assign) else [node.target]
        for n in ast.walk(targets[0]):
            if isinstance(n, ast.Name):
                return n.id
        return ast.unparse(targets[0])
    if isinstance(node, ast.Expr):
        value = node.value.value if isinstance(node.value, ast.Await) else node.value
        if isinstance(value, ast.Call):
            return ast.unparse(value.func)
    return type(node).__name__.lower()


def _bound_args(node: ast.stmt) -> set[str]:
    out: set[str] = set()
    for n in ast.walk(node):
        if isinstance(n, ast.arg):
            out.add(n.arg)
    return out


def _local_imports(tree: ast.Module, files: set[str]) -> dict[str, str]:
    """Map each name imported from a sibling module to ``<path>:<name>``."""
    out: dict[str, str] = {}
    for node in tree.body:
        if isinstance(node, ast.ImportFrom) and node.module and node.level == 0:
            path = node.module.replace(".", "/") + ".py"
            if path in files:
                for alias in node.names:
                    out[alias.asname or alias.name] = f"{path}:{alias.name}"
    return out


def _decompile_file(
    path: str, source: str, files: set[str], patterns: list[Snippet]
) -> tuple[dict, list[dict]]:
    tree = ast.parse(source)
    lines = source.splitlines()
    body = list(tree.body)
    docstring = None
    prev_end = 0
    if body and isinstance(body[0], ast.Expr) and isinstance(body[0].value, ast.Constant) \
            and isinstance(body[0].value.value, str):
        segment = ast.get_source_segment(source, body[0])
        docstring = body[0].value.value
        if segment != f'"""{docstring}"""':
            raise DecompileError(f"{path}: module docstring is not a plain triple-double-quoted string")
        prev_end = body[0].end_lineno or 0
        body = body[1:]

    ops: list[dict] = [{"op": "CREATE_FILE", "path": path, "docstring": docstring}]
    group = 0
    while body and isinstance(body[0], (ast.Import, ast.ImportFrom)):
        node = body.pop(0)
        if len(ops) > 1 and any(not lines[i].strip() for i in range(prev_end, node.lineno - 1)):
            group += 1
        stmt = ast.get_source_segment(source, node)
        ops.append({"op": "ADD_IMPORT", "path": path, "stmt": stmt, "group": group})
        prev_end = node.end_lineno or node.lineno

    local = _local_imports(tree, files)
    counts: dict[str, int] = {}
    names: list[str] = []
    for node in body:
        base = _block_base_name(node)
        counts[base] = counts.get(base, 0) + 1
        names.append(base if counts[base] == 1 else f"{base}#{counts[base]}")
    first_of = {}
    for node, name in zip(body, names):
        first_of.setdefault(_block_base_name(node), name)

    blocks: list[dict] = []
    for index, (node, name) in enumerate(zip(body, names)):
        start = prev_end
        while start < len(lines) and not lines[start].strip():
            start += 1
        blank_before = start - prev_end
        end = node.end_lineno or node.lineno
        if index == len(body) - 1:
            end = len(lines)
        text = "\n".join(lines[start:end]).rstrip("\n")
        prev_end = node.end_lineno or node.lineno

        shadowed = _bound_args(node)
        refs: list[str] = []
        loads = sorted(
            (n for n in ast.walk(node) if isinstance(n, ast.Name) and n.id not in shadowed),
            key=lambda n: (n.lineno, n.col_offset),
        )
        for n in loads:
            dep = first_of.get(n.id) if n.id in first_of else local.get(n.id)
            if dep and dep != name and dep not in refs:
                refs.append(dep)
        blocks.append({"name": name, "kind": _block_kind(node), "depends_on": refs})

        op: dict = {"op": "INSERT_BLOCK", "path": path, "block": name, "source": text,
                    "blank_before": blank_before}
        for snippet in patterns:
            params = snippet.match(text)
            if params is not None:
                op = {"op": "INSERT_SNIPPET", "path": path, "block": name, "id": snippet.id,
                      "params": params, "blank_before": blank_before}
                break
        ops.append(op)
    return {"path": path, "blocks": blocks}, ops


def decompile(files: dict[str, str]) -> dict:
    """Return ``{topology, ops, coverage}`` for a program given as ``{path: source}``."""
    patterns = _snippet_patterns()
    topology: dict = {"files": []}
    ops: list[dict] = []
    for path in sorted(files):
        entry, file_ops = _decompile_file(path, files[path], set(files), patterns)
        topology["files"].append(entry)
        ops.extend(file_ops)
    return {
        "topology": topology,
        "ops": ops,
        "coverage": {
            "snippet_blocks": sum(1 for op in ops if op["op"] == "INSERT_SNIPPET"),
            "literal_blocks": sum(1 for op in ops if op["op"] == "INSERT_BLOCK"),
        },
    }


def read_program(root: Path) -> dict[str, str]:
    """Every ``.py`` file under ``root``, keyed by its relative path."""
    return {
        p.relative_to(root).as_posix(): p.read_text(encoding="utf-8")
        for p in sorted(root.rglob("*.py"))
        if "__pycache__" not in p.parts
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
    ap.add_argument("--path", type=Path, help="decompile the multi-file program under this directory and print its records")
    ap.add_argument("--corpus", type=Path, default=CORPUS, help="corpus layer to read (default data/curated; data/user for your own families)")
    ap.add_argument("families", nargs="*", help="family number prefixes to limit to, e.g. 01 48")
    args = ap.parse_args()
    if args.path:
        print(json.dumps(decompile(read_program(args.path)), indent=2, ensure_ascii=False))
        return 0
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
