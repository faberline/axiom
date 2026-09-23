#!/usr/bin/env python3
"""Verify the curation layer of an executable oracle corpus (default ``data/curated``).

The harness (``scripts/verify_harness.py``) proves the
behavioral contract by execution. This script proves, without executing any
candidate, that the curation written into each ``family.json`` is present,
non-trivial, and bound to the fixture and the gold source it claims:

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
- ``caption`` is the long description of the gold program, the planner's
  target: between ``MIN_WORDS_CAPTION`` and ``MAX_WORDS_CAPTION`` words, no
  label word, not the ``requirement`` pasted in, and honest about the gold
  source: it names every exception class the gold raises, every status code
  it declares, and every third-party library it imports, and it names no
  exception class that neither the gold nor the fixture mentions.
- ``decompiled`` equals what ``decompile_gold.measure`` reads from the gold
  source today, so the surface, imports, raises, and status codes stored in
  the record cannot drift from the code, the discipline ``caught_by`` follows.

Exit 0 when every family passes; exit 1 with one line per defect otherwise.
"""

from __future__ import annotations

import argparse
import ast
import io
import json
import re
import sys
import tokenize
from collections import Counter
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from decompile_gold import measure  # noqa: E402

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
MISS_IDS = ["miss_1", "miss_2", "miss_3", "miss_4", "miss_5"]
MIN_WORDS_RATIONALE = 8
MIN_WORDS_WHY_WRONG = 6
MIN_WORDS_CAPTION = 80
MAX_WORDS_CAPTION = 400
LABEL_RE = re.compile(r"\bgold\b|near.?miss|failure.?mode|\bmiss[ _][0-9]", re.IGNORECASE)
CAPTION_EXC_RE = re.compile(r"\b[A-Z][A-Za-z0-9]*(?:Error|Exception|Exit)\b")
IMPORT_ALIASES = {
    "bs4": ("beautifulsoup",),
    "jwt": ("pyjwt",),
    "yaml": ("pyyaml",),
    "sklearn": ("scikit-learn",),
}


def words(text: str) -> int:
    return len(text.split())


def normalize(text: str) -> str:
    return " ".join(text.lower().split())


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


def check_caption(name: str, meta: dict, gold_src: str | None, fixture_src: str) -> list[str]:
    """Caption honesty: bounded, label-free, not the requirement, grounded in the gold."""
    defects: list[str] = []
    caption = meta.get("caption")
    if not isinstance(caption, str):
        return [f"{name}: caption missing"]
    n = words(caption)
    if n < MIN_WORDS_CAPTION or n > MAX_WORDS_CAPTION:
        defects.append(f"{name}: caption has {n} words, outside {MIN_WORDS_CAPTION}..{MAX_WORDS_CAPTION}")
    hit = LABEL_RE.search(caption)
    if hit:
        defects.append(f"{name}: label word {hit.group(0)!r} in caption")
    if normalize(meta.get("requirement", "")) in normalize(caption):
        defects.append(f"{name}: caption contains the requirement verbatim")
    if gold_src is None:
        return defects
    grounding = gold_src + "\n" + fixture_src
    for exc in sorted(set(CAPTION_EXC_RE.findall(caption))):
        if not re.search(rf"\b{re.escape(exc)}\b", grounding):
            defects.append(f"{name}: caption names {exc}, which neither the gold nor the fixture mentions")
    measured = measure(gold_src)
    for exc in measured["raises"]:
        if not re.search(rf"\b{re.escape(exc)}\b", caption):
            defects.append(f"{name}: caption does not name the raised {exc}")
    for code in measured["status_codes"]:
        if not re.search(rf"\b{code}\b", caption):
            defects.append(f"{name}: caption does not name status code {code}")
    low = caption.lower()
    for mod in measured["imports"]:
        if not any(alias in low for alias in (mod.lower(), *IMPORT_ALIASES.get(mod, ()))):
            defects.append(f"{name}: caption does not name the imported library {mod}")
    return defects


def check_decompiled(name: str, meta: dict, gold_src: str | None) -> list[str]:
    """The stored measurement equals a fresh one; report the keys that moved."""
    if gold_src is None:
        return []
    measured = measure(gold_src)
    stored = meta.get("decompiled")
    if stored == measured:
        return []
    if not isinstance(stored, dict):
        return [f"{name}: decompiled missing"]
    moved = [k for k in measured if stored.get(k) != measured[k]]
    extra = [k for k in stored if k not in measured]
    return [f"{name}: decompiled differs from the gold measurement in {', '.join(moved + extra)}"]


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
    fixture_src = fixture.read_text(encoding="utf-8") if fixture else ""

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
    defects.extend(check_caption(name, meta, sources.get("gold"), fixture_src))
    defects.extend(check_decompiled(name, meta, sources.get("gold")))
    return defects


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--corpus", type=Path, default=CORPUS, help="corpus layer to read (default data/curated; data/user for your own families)")
    use_corpus(ap.parse_args().corpus)
    families = sorted(d for d in MATERIALS.iterdir() if (d / "family.json").is_file())
    if not families:
        print(f"no families under {MATERIALS}")
        return 1
    defects: list[str] = []
    modes: Counter[str] = Counter()
    with_oracle = captioned = 0
    for fam in families:
        defects.extend(check_family(fam))
        meta = json.loads((fam / "family.json").read_text(encoding="utf-8"))
        with_oracle += meta.get("oracle") is not None
        captioned += isinstance(meta.get("caption"), str)
        modes.update(c.get("failure_mode") for c in meta["candidates"] if c["kind"] != "gold")
    print(
        f"families: {len(families)}  with oracle: {with_oracle}  captioned: {captioned}"
        f"  near misses: {sum(modes.values())}"
    )
    for mode, count in modes.most_common():
        print(f"  {mode}: {count}")
    for line in defects:
        print(f"DEFECT {line}")
    print(f"[RESULT: {'SUCCESS' if not defects else 'FAILURE'}] {len(defects)} curation defects")
    return 0 if not defects else 1


if __name__ == "__main__":
    sys.exit(main())
