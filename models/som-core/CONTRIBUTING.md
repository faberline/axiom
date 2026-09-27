# Contributing to SOM core

## Brief

How to change `models/som-core`. What it promises lives in
[README.md](README.md); the per-surface support state is in
[STATUS.md](STATUS.md); repository-wide authoring and verification rules live
in the root [CONTRIBUTING.md](../../CONTRIBUTING.md).

The project is a Python package under `src/som_core/` with the `som` entry
point declared in `pyproject.toml`, with its tests under `tests/`. Create
the environment with `uv sync` run inside `models/som-core`, which also
installs the `dev` group that carries pytest; every command below runs from
the repository root against that environment. `som verify` needs Apple
MLX, so that gate runs on macOS with Apple silicon only.

There is no `aw` phase ladder for `models/` yet, so a change here is verified
by hand: run the gates below before and after, and keep the STATUS matrix true
to what they observe. `runs/` is local output and never committed.

## Verification

| Gate | Command |
|---|---|
| CLI surface | `models/som-core/.venv/bin/som --help` |
| Loader, records, assembler | `models/som-core/.venv/bin/python -m pytest models/som-core/tests -q` |
| MLX device | `models/som-core/.venv/bin/som verify` |
| Corpus round trip | `uv run --project models/som-code-python python models/som-code-python/scripts/verify_roundtrip.py` |
| Product document contract | `uv run --python 3.13 --no-project scripts/meta/project_docs_contract.py check models/som-core --format json` |
| META-doc contract | `uv run --project apps/aw aw metadoc check models/som-core` |

Run the two document checks after editing `README.md`, `STATUS.md`,
`ROADMAP.md`, or anything under `docs/product/`.

A change to `records.py` or `assemble.py` also runs the corpus round trip,
which reassembles every curated family and runs its fixture.
