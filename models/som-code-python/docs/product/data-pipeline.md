# Data pipeline

What turns families into training records and keeps the project runnable:
the test suite, the decompiler that emits the layer records, packaging, and
the retirement of tests that read data this tree no longer carries.

## Project test suite

- Problem: none open as shipped; the limits below belong to the last outcome
  in this area.
- Who: corpus authors changing a script under `som/` or `scripts/`.
- Promise: `tests/` runs under the project virtualenv, and the oracle,
  candidate quality, corpus materializer, seed inventory, training runtime,
  and snippet adversarial cases pass.
- Limits today: the tests named in the STATUS row `python-test-suite` fail
  because the legacy corpora, runtimes, and lock files they read are not
  tracked here, so the suite as a whole exits non-zero.
- Non-goals: restoring the legacy data those tests read (ROADMAP
  `legacy-ranker-lineage`).
- Neighbours: none; first section of the area.
- Status rows: `python-test-suite`.

## Decompiler DSL corpus

- Problem: A family is a gold program and five near misses; nothing records
  which intent, which files and blocks, and which snippet operations produced
  the gold, so the engine's planner, topology, and optimizer layers have no
  rows to learn from.
- Who: model trainers; generator authors.
- Promise: A decompiler turns every family into its L1 intent, L2 component
  topology, and L3 operation sequence records, distinguishes gold from near
  miss with the failure mode, and reassembles the multi-file TODO fixture
  from its records with no AST loss.
- Non-goals: the engine's loader; a decompiler for another language.
- Open: the test module and its TODO fixture sit at the repository root under
  `tests/data_pipeline/` and `tests/fixtures/multi_file_todo/`, beside
  `ORIGINAL_REQUEST.md`, the `extract_*payload*.py` scripts, `chunk`, and
  `scripts/generator/`; which of these move into this project and which are
  deleted is a release-plan decision, as is whether the records are one JSON
  document per family or one JSONL line per layer, since the engine's loader
  outcome consumes whichever is chosen.
- Neighbours: [Project test suite](#project-test-suite) above, which the
  moved tests join; the corpus area, whose families are the input.
- Outcome: `decompiler-dsl-corpus`. Tracking: Not assigned.

## uv-runnable packaging

- Problem: `uv run --project models/som-code-python` cannot resolve
  `som-core`, and `pyproject.toml` looks for the package under `src/` while
  the code is under `som/`, so every gate runs against a hand-made
  virtualenv.
- Who: corpus authors; anyone running the gates on a fresh checkout.
- Promise: `uv run --project models/som-code-python` resolves `som-core`
  from its sibling path and imports `som` from where it lives, with no code
  moved.
- Non-goals: publishing either package; moving `som/` under `src/`.
- Open: whether the `som` console script this project declares is kept or
  dropped in favour of the engine's, since both projects name the same entry
  point today.
- Neighbours: [Decompiler DSL corpus](#decompiler-dsl-corpus) above, whose
  tests should run through the same `uv` entry.
- Outcome: `uv-runnable-packaging`. Tracking: Not assigned.

## Legacy suite retired

- Problem: The suite exits non-zero on tests that read corpora, runtimes, and
  lock files the legacy ranker research left behind, so a red gate says
  nothing about a change to the corpus.
- Who: corpus authors, who need a green gate to mean something.
- Promise: Every test that reads legacy data not tracked here is removed or
  moved with that data, and the pytest gate exits 0 with no deselection.
- Non-goals: restoring the legacy data; weakening the tests that pass today.
- Open: whether the modules only those tests exercise (`som/specialists`,
  `som/developer`, and the runtime trees) leave the tree with them.
- Neighbours: [Project test suite](#project-test-suite) above, whose Limits
  this removes.
- Outcome: `legacy-suite-retired`. Tracking: Not assigned.

## Non-goals in this area

- No restoration of the legacy corpora, runtimes, seed adapters, or lock
  files; see ROADMAP `legacy-ranker-lineage`.
- No pipeline for a platform without a Python oracle; see ROADMAP
  `platform-pilot-families`.
