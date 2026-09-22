# Contributing to the SOM code corpus for Python

## Brief

How to change `models/som-code-python`. What it promises lives in
[README.md](README.md); the per-surface support state is in
[STATUS.md](STATUS.md); repository-wide authoring and verification rules live
in the root [CONTRIBUTING.md](../../CONTRIBUTING.md).

The project is data plus the scripts that verify it. A family is a directory
under `data/python-v2/materials/` with `family.json`, `gold`, and `miss_1`
through `miss_5`, judged by the fixture of the same number under
`data/python-v2/fixtures/`. Write the fixture first and watch every near miss
fail on an assertion before the family counts. The `family.json` also
carries the family's rationale: `rationale.teaches`, `rationale.why`, the
`oracle` fixture path, and for each near miss `why_wrong` and `caught_by`.
Write `why_wrong` as the observable consequence of the defect, never the
failure mode's name; take `caught_by` from the harness, which prints the
declared and the actual failing tests whenever they differ. A snippet is one JSON file
under `data/snippets/v1/<library>/` and is not in the ISA until the verifier's
required set names it.

The gates run from the repository root against the project virtualenv at
`models/som-code-python/.venv`. `uv sync` inside the project does not resolve
today (STATUS row `uv-runnable-packaging`), so the virtualenv is created by
hand and is not committed. There is no `aw` phase ladder for `models/` yet, so
a change here is verified by running the gates below before and after and
keeping the STATUS matrix true to what they observe.

## Verification

| Gate | Command |
|---|---|
| Oracle corpus, every family | `models/som-code-python/.venv/bin/python models/som-code-python/data/python-v2/fixtures/verify_harness.py` |
| Curated rationale | `models/som-code-python/.venv/bin/python models/som-code-python/scripts/verify_curation.py` |
| Snippet ISA v1 | `models/som-code-python/.venv/bin/python models/som-code-python/scripts/verify_snippets.py` |
| Project test suite | `models/som-code-python/.venv/bin/python -m pytest models/som-code-python/tests -q` |
| Product document contract | `uv run --python 3.13 --no-project scripts/meta/project_docs_contract.py check models/som-code-python --format json` |
| META-doc contract | `uv run --project apps/aw aw metadoc check models/som-code-python` |

Run the two document checks after editing `README.md`, `STATUS.md`,
`ROADMAP.md`, or anything under `docs/product/`. Run the harness and the
curation check after editing anything under `data/python-v2/`. The harness accepts family
prefixes as arguments to scope a run, but the gate above is the unscoped one.
