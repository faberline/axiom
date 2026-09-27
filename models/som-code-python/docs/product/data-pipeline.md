# Data pipeline

What turns families into training records and keeps the project runnable:
the test suite, packaging, and the decompiled layer records with the round
trip that proves them.

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

## Decompiled layer records

- Problem: none open as shipped; the snippet share is limited, see below.
- Who: model trainers, whose SFT pairs are caption to plan, plan to
  topology, and topology to operation list; generator authors, who need to
  know how much of each gold the ISA expresses.
- Promise: Every family carries an authored L1 plan and the L2 topology, L3
  operation list, and snippet coverage measured from its gold, and `som
  assemble` rebuilds every gold, and the multi-file TODO fixture, from those
  records to the same AST, with the family's fixture passing on the rebuilt
  file. `scripts/decompile_gold.py --write` measures the records and the
  curation check refuses drift, a plan whose constraints omit a raised
  exception or declared status code, and a topology over the engine's scope
  limit.
- Limits today: most blocks are still `INSERT_BLOCK` literals;
  `decompiled.coverage` reports the share per family, STATUS `snippet-isa`
  the corpus total, and ROADMAP `snippet-long-tail` raises it.
- Non-goals: decompiling near misses, which belongs to the engine's
  preference-data outcome; a decompiler for another language.
- Neighbours: [Project test suite](#project-test-suite) above, which holds
  the multi-file TODO fixture; the corpus area, whose families are the
  input; the engine's docs/reference/layer-records.md, which fixes the
  schema.
- Status rows: `decompiled-layer-records`.

## Non-goals in this area

- No pipeline for a platform without a Python oracle; see ROADMAP
  `platform-pilot-families`.
