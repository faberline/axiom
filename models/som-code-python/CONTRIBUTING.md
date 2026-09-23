# Contributing to the SOM code corpus for Python

## Brief

How to change `models/som-code-python`. What it promises lives in
[README.md](README.md); the per-surface support state is in
[STATUS.md](STATUS.md); repository-wide authoring and verification rules live
in the root [CONTRIBUTING.md](../../CONTRIBUTING.md).

The project is data plus the scripts that verify it. A family is a directory
under `data/curated/families/` with `family.json`, `gold`, and `miss_1`
through `miss_5`, judged by the fixture of the same number under
`data/curated/fixtures/`. Write the fixture first and watch every near miss
fail on an assertion before the family counts. The `family.json` also
carries the family's rationale: `rationale.teaches`, `rationale.why`, the
`oracle` fixture path, and for each near miss `why_wrong` and `caught_by`.
Write `why_wrong` as the observable consequence of the defect, never the
failure mode's name; take `caught_by` from the harness, which prints the
declared and the actual failing tests whenever they differ. Write `caption`
from the gold source, 80 to 400 words, naming every exception it raises and
library it imports and ending with what the fixture asserts; run
`scripts/decompile_gold.py --write` after changing a gold candidate so
`decompiled` matches it, and `--draft <dir>` for a per-family sheet to write
the caption from. Write `plan` beside the caption: an imperative `intent`,
the domain `target`, and `constraints` that name every exception the gold
raises and status code it declares, never the requirement text. A snippet is one JSON file
under `data/snippets/<library>/` and is not in the ISA until the verifier's
required set names it.

Every candidate is written as modern, PEP 8, typed Python: builtin generics
and `X | None`, `collections.abc` imports, docstrings on modules, classes,
and public functions, `raise ... from` inside `except`, and no unused import
or argument. Fix the gold until the quality gate is silent, then apply the
same edit to all five near misses so each stays a minimal edit of its gold
and shares its comments and docstrings. A suppression comment is the last
resort and says why on the same line.

Create the virtualenv with `uv sync --project models/som-code-python`; the
gates run from the repository root through `uv run`. There is no `aw` phase
ladder for `models/` yet, so a change here is verified by running the gates
below before and after and keeping the STATUS matrix true to what they
observe.

## Verification

| Gate | Command |
|---|---|
| Oracle corpus, every family | `uv run --project models/som-code-python python models/som-code-python/scripts/verify_harness.py` |
| Curated rationale | `uv run --project models/som-code-python python models/som-code-python/scripts/verify_curation.py` |
| Code quality | `uv run --project models/som-code-python python models/som-code-python/scripts/verify_quality.py` |
| Round trip | `uv run --project models/som-code-python python models/som-code-python/scripts/verify_roundtrip.py` |
| Snippet ISA v1 | `uv run --project models/som-code-python python models/som-code-python/scripts/verify_snippets.py` |
| Project test suite | `uv run --project models/som-code-python python -m pytest models/som-code-python/tests -q` |
| Product document contract | `uv run --python 3.13 --no-project scripts/meta/project_docs_contract.py check models/som-code-python --format json` |
| META-doc contract | `uv run --project apps/aw aw metadoc check models/som-code-python` |

Run the two document checks after editing `README.md`, `STATUS.md`,
`ROADMAP.md`, or anything under `docs/product/`. Run the harness, the
curation check, the quality gate, and the round trip after editing anything
under `data/curated/`. The harness accepts family
prefixes as arguments to scope a run, but the gate above is the unscoped one.
