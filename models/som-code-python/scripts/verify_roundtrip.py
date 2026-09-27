#!/usr/bin/env python3
"""Prove each family's stored layer records rebuild its gold program.

For every family, ``decompiled.topology`` and ``decompiled.ops`` go through
the reference assembler (``som assemble``, run as a subprocess from
``models/som-core``), and the assembled ``candidate.py`` must:

- parse to the same ``ast.dump`` as the gold, and
- pass the family's fixture, run exactly as ``scripts/verify_harness.py``
  runs a gold candidate.

The multi-file program under ``tests/fixtures/multi_file_todo`` is
decompiled with ``decompile_gold.decompile`` and assembled the same way;
each of its files must match on ``ast.dump`` (it has no fixture). A family
without a fixture is checked on ``ast.dump`` alone, and a family whose
topology is over the planner's scope is refused by the assembler as
``SCOPE_TOO_LARGE``; both are listed by name on every run. How many
assembled files are byte-identical to their source is reported, not gated:
comments between imports and blank-line runs are not part of the records.
"""

from __future__ import annotations

import argparse
import ast
import json
import subprocess
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from decompile_gold import GOLD_PATH, decompile, read_program  # noqa: E402
from verify_harness import (  # noqa: E402
    DATA,
    default_repo_root,
    discover_families,
    find_fixture_test,
    find_python_executable,
    run_candidate_test,
)

PROJECT = Path(__file__).resolve().parents[1]
SOM_CORE = PROJECT.parent / "som-core"
MULTI_FILE = PROJECT / "tests" / "fixtures" / "multi_file_todo"


def som_assemble(document: dict, out: Path) -> tuple[int, str]:
    """Run ``som assemble`` on ``{topology, ops}``; return its exit code and stderr."""
    doc_path = out.parent / f"{out.name}.json"
    doc_path.write_text(json.dumps(document), encoding="utf-8")
    proc = subprocess.run(
        ["uv", "run", "--quiet", "--project", str(SOM_CORE), "som", "assemble", str(doc_path),
         "--out", str(out), "--isa", str(DATA / "snippets")],
        capture_output=True, text=True, check=False,
    )
    return proc.returncode, proc.stderr.strip()


def same_ast(a: str, b: str) -> bool:
    return ast.dump(ast.parse(a)) == ast.dump(ast.parse(b))


def check_family(
    fam: Path, fixtures: Path, python_exe: str, scratch: Path, notes: dict[str, list[str]]
) -> tuple[list[str], bool]:
    """Return the family's defects and whether the assembled file is byte-identical."""
    meta = json.loads((fam / "family.json").read_text(encoding="utf-8"))
    gold = next(c for c in meta["candidates"] if c["kind"] == "gold")
    gold_src = (fam / gold["module"]).read_text(encoding="utf-8")
    block = meta.get("decompiled") or {}
    if "ops" not in block or "topology" not in block:
        return [f"{fam.name}: decompiled has no topology/ops"], False
    out = scratch / fam.name
    code, err = som_assemble({"topology": block["topology"], "ops": block["ops"]}, out)
    if code != 0 and "SCOPE_TOO_LARGE" in err:
        notes["scope"].append(f"{fam.name} ({err.split('SCOPE_TOO_LARGE: ', 1)[-1]})")
        return [], False
    if code != 0:
        return [f"{fam.name}: som assemble exited {code}: {err}"], False
    built = (out / GOLD_PATH).read_text(encoding="utf-8")
    defects = []
    if not same_ast(built, gold_src):
        defects.append(f"{fam.name}: assembled candidate.py differs from the gold AST")
    test_file = find_fixture_test(fixtures, fam.name)
    if test_file is None:
        notes["ast_only"].append(fam.name)
    else:
        rc, stdout, stderr = run_candidate_test(python_exe, default_repo_root(), out, test_file)
        if rc != 0:
            tail = (stdout or stderr).strip().splitlines()[-1:] or [""]
            defects.append(f"{fam.name}: fixture exited {rc} on the assembled file: {tail[0]}")
    return defects, built == gold_src


def check_multi_file(scratch: Path) -> tuple[list[str], int, int]:
    program = read_program(MULTI_FILE)
    records = decompile(program)
    out = scratch / "multi_file_todo"
    code, err = som_assemble({"topology": records["topology"], "ops": records["ops"]}, out)
    if code != 0:
        return [f"multi_file_todo: som assemble exited {code}: {err}"], 0, len(program)
    defects, identical = [], 0
    for path, source in program.items():
        built = (out / path).read_text(encoding="utf-8")
        identical += built == source
        if not same_ast(built, source):
            defects.append(f"multi_file_todo/{path}: assembled file differs from the source AST")
    return defects, identical, len(program)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--corpus", type=Path, default=DATA / "curated", help="corpus layer to read (default data/curated; data/user for your own families)")
    ap.add_argument("families", nargs="*", help="family number prefixes to limit to, e.g. 01 48")
    args = ap.parse_args()

    fixtures = (args.corpus / "fixtures").resolve()
    selected = discover_families((args.corpus / "families").resolve(), args.families or None)
    if not selected:
        print(f"no families under {args.corpus}")
        return 1
    python_exe = find_python_executable(default_repo_root())
    defects: list[str] = []
    notes: dict[str, list[str]] = {"scope": [], "ast_only": []}
    identical = 0
    with tempfile.TemporaryDirectory(prefix="som_roundtrip_") as tmp:
        scratch = Path(tmp)
        for fam in selected:
            fam_defects, same_bytes = check_family(fam, fixtures, python_exe, scratch, notes)
            defects.extend(fam_defects)
            identical += same_bytes
        multi_defects, multi_identical, multi_total = ([], 0, 0)
        if not args.families:
            multi_defects, multi_identical, multi_total = check_multi_file(scratch)
            defects.extend(multi_defects)

    for line in defects:
        print(f"[-] {line}")
    print(f"ast only (no fixture): {', '.join(notes['ast_only']) or 'none'}")
    print(f"over scope (SCOPE_TOO_LARGE): {', '.join(notes['scope']) or 'none'}")
    print(f"families: {len(selected)}  byte-identical: {identical}/{len(selected)}", end="")
    print(f"  multi_file_todo byte-identical: {multi_identical}/{multi_total}" if multi_total else "")
    if defects:
        print(f"[RESULT: FAILURE] {len(defects)} defects")
        return 1
    print("[RESULT: SUCCESS] every in-scope family rebuilds its gold AST and passes its fixture")
    return 0


if __name__ == "__main__":
    sys.exit(main())
