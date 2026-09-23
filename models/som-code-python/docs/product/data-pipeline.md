# Data pipeline

What turns families into training records and keeps the project runnable:
the test suite, the decompiler that emits the layer records, and packaging.

## Project test suite

- Problem: none open as shipped.
- Who: corpus authors changing a script under `scripts/`.
- Promise: `tests/` runs under the project virtualenv and the snippet
  adversarial cases pass, so the pytest gate exits 0 with no deselection.
- Non-goals: tests for the ranker research that predates the layer model;
  that code is gone from this tree.
- Neighbours: none; first section of the area.
- Status rows: `python-test-suite`.

## uv-runnable packaging

- Problem: none open as shipped.
- Who: corpus authors; anyone running the gates on a fresh checkout.
- Promise: `uv sync --project models/som-code-python` resolves from the lock
  file and installs every library the candidates and fixtures import plus
  the quality tools, and every gate runs through `uv run --project
  models/som-code-python`.
- Non-goals: publishing a package; a console script, which the engine in
  `models/som-core` owns.
- Neighbours: [Project test suite](#project-test-suite) above, which runs
  through the same entry.
- Status rows: `uv-runnable-packaging`.

## Decompiler DSL corpus

- Problem: A family is a gold program and five near misses. Each
  `family.json` now carries a `caption` (the planner's prompt) and a
  `decompiled.surface` measured from the gold AST (the component list), but
  nothing records which files and blocks and which snippet operations
  produced the gold, so the engine's topology and optimizer layers have no
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
- Neighbours: [Project test suite](#project-test-suite) and [uv-runnable
  packaging](#uv-runnable-packaging) above, which the moved tests join; the
  corpus area, whose families are the input.
- Outcome: `decompiler-dsl-corpus`. Tracking: Not assigned.

## Non-goals in this area

- No pipeline for a platform without a Python oracle; see ROADMAP
  `platform-pilot-families`.
