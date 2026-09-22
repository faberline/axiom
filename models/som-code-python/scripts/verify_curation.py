#!/usr/bin/env python3
"""Verify the curation layer of the python-v2 executable oracle corpus.

The harness (``data/python-v2/fixtures/verify_harness.py``) proves the
behavioral contract by execution. This script proves, without executing any
candidate, that the curation written into each ``family.json`` is present,
non-trivial, and bound to the fixture it claims:

- ``rationale.teaches`` and ``rationale.why`` exist and are real sentences.
- ``oracle`` names an existing fixture, or is null exactly when no fixture
  exists for that family number.
- Every near miss carries ``why_wrong`` (a sentence about the observable
  consequence, not an echo of ``failure_mode``) and ``caught_by`` (a sorted,
  de-duplicated list of test functions defined in that fixture, or null when
  there is no fixture).
- The gold candidate carries neither field.
- Comments and docstrings are identical across the six candidates, so no
  candidate can be told apart by prose instead of behavior.
- No candidate mentions a label word (gold, near miss, failure mode).

Exit 0 when every family passes; exit 1 with one line per defect otherwise.
"""

from __future__ import annotations

import ast
import io
import json
import re
import sys
import tokenize
from collections import Counter
from pathlib import Path

CORPUS = Path(__file__).resolve().parent.parent / "data" / "python-v2"
MATERIALS = CORPUS / "materials"
FIXTURES = CORPUS / "fixtures"
MISS_IDS = ["miss_1", "miss_2", "miss_3", "miss_4", "miss_5"]
MIN_WORDS_RATIONALE = 8
MIN_WORDS_WHY_WRONG = 6
LABEL_RE = re.compile(r"\bgold\b|near.?miss|failure.?mode|\bmiss[ _][0-9]", re.IGNORECASE)


def words(text: str) -> int:
    return len(text.split())


def comment_multiset(src: str) -> Counter[str]:
    out: Counter[str] = Counter()
    for tok in tokenize.generate_tokens(io.StringIO(src).readline):
        if tok.type == tokenize.COMMENT:
            out[tok.string] += 1
    return out


def docstrings(src: str) -> dict[str, str]:
    tree = ast.parse(src)
    found: dict[str, str] = {}
    module_doc = ast.get_docstring(tree, clean=False)
    if module_doc is not None:
        found["<module>"] = module_doc

    def visit(node: ast.AST, prefix: str) -> None:
        for child in ast.iter_child_nodes(node):
            if isinstance(child, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)):
                name = f"{prefix}.{child.name}" if prefix else child.name
                doc = ast.get_docstring(child, clean=False)
                if doc is not None:
                    found[name] = doc
                visit(child, name)

    visit(tree, "")
    return found


def fixture_test_names(path: Path) -> set[str]:
    tree = ast.parse(path.read_text(encoding="utf-8"))
    return {
        node.name
        for node in ast.walk(tree)
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)) and node.name.startswith("test")
    }


def fixture_for(family_name: str) -> Path | None:
    prefix = family_name.split("-")[0]
    hits = sorted(FIXTURES.glob(f"test_{prefix}_*.py"))
    return hits[0] if hits else None


def check_family(fam: Path) -> list[str]:
    defects: list[str] = []
    name = fam.name
    meta = json.loads((fam / "family.json").read_text(encoding="utf-8"))

    rationale = meta.get("rationale")
    if not isinstance(rationale, dict):
        defects.append(f"{name}: rationale missing")
    else:
        for key in ("teaches", "why"):
            text = rationale.get(key)
            if not isinstance(text, str) or words(text) < MIN_WORDS_RATIONALE:
                defects.append(f"{name}: rationale.{key} missing or under {MIN_WORDS_RATIONALE} words")

    fixture = fixture_for(name)
    oracle = meta.get("oracle", "missing")
    if oracle == "missing":
        defects.append(f"{name}: oracle key missing")
    elif fixture is None and oracle is not None:
        defects.append(f"{name}: oracle {oracle!r} declared but no fixture exists")
    elif fixture is not None and oracle != f"fixtures/{fixture.name}":
        defects.append(f"{name}: oracle {oracle!r} != fixtures/{fixture.name}")
    test_names = fixture_test_names(fixture) if fixture else set()

    cands = {c["id"]: c for c in meta.get("candidates", [])}
    if sorted(cands) != sorted(["gold", *MISS_IDS]):
        defects.append(f"{name}: candidates are {sorted(cands)}, expected gold + miss_1..miss_5")
    gold = cands.get("gold", {})
    for key in ("why_wrong", "caught_by"):
        if key in gold:
            defects.append(f"{name}: gold carries {key}")
    for mid in MISS_IDS:
        cand = cands.get(mid)
        if cand is None:
            continue
        mode = cand.get("failure_mode", "")
        why = cand.get("why_wrong")
        if not isinstance(why, str) or words(why) < MIN_WORDS_WHY_WRONG:
            defects.append(f"{name}/{mid}: why_wrong missing or under {MIN_WORDS_WHY_WRONG} words")
        elif mode and (mode in why or mode.replace("_", " ") in why.lower()):
            defects.append(f"{name}/{mid}: why_wrong echoes failure_mode {mode!r}")
        if "caught_by" not in cand:
            defects.append(f"{name}/{mid}: caught_by missing")
            continue
        caught = cand["caught_by"]
        if fixture is None:
            if caught is not None:
                defects.append(f"{name}/{mid}: caught_by must be null without a fixture")
            continue
        if not isinstance(caught, list) or not caught:
            defects.append(f"{name}/{mid}: caught_by must be a non-empty list")
            continue
        if caught != sorted(set(caught)):
            defects.append(f"{name}/{mid}: caught_by is not sorted and unique")
        for t in caught:
            if t not in test_names:
                defects.append(f"{name}/{mid}: caught_by names {t!r}, not defined in {fixture.name}")

    sources = {cid: (fam / c["module"]).read_text(encoding="utf-8") for cid, c in cands.items()}
    for cid, src in sources.items():
        hit = LABEL_RE.search(src)
        if hit:
            defects.append(f"{name}/{cid}: label word {hit.group(0)!r} in candidate source")
    if "gold" in sources:
        gold_comments = comment_multiset(sources["gold"])
        gold_docs = docstrings(sources["gold"])
        for cid, src in sources.items():
            if cid == "gold":
                continue
            if comment_multiset(src) != gold_comments:
                defects.append(f"{name}/{cid}: comments differ from gold")
            docs = docstrings(src)
            for key in gold_docs.keys() & docs.keys():
                if docs[key] != gold_docs[key]:
                    defects.append(f"{name}/{cid}: docstring of {key} differs from gold")
    return defects


def main() -> int:
    families = sorted(d for d in MATERIALS.iterdir() if (d / "family.json").is_file())
    if not families:
        print(f"no families under {MATERIALS}")
        return 1
    defects: list[str] = []
    modes: Counter[str] = Counter()
    with_oracle = 0
    for fam in families:
        defects.extend(check_family(fam))
        meta = json.loads((fam / "family.json").read_text(encoding="utf-8"))
        with_oracle += meta.get("oracle") is not None
        modes.update(c.get("failure_mode") for c in meta["candidates"] if c["kind"] != "gold")
    print(f"families: {len(families)}  with oracle: {with_oracle}  near misses: {sum(modes.values())}")
    for mode, count in modes.most_common():
        print(f"  {mode}: {count}")
    for line in defects:
        print(f"DEFECT {line}")
    print(f"[RESULT: {'SUCCESS' if not defects else 'FAILURE'}] {len(defects)} curation defects")
    return 0 if not defects else 1


if __name__ == "__main__":
    sys.exit(main())
