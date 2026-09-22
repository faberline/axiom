# Corpus

What a model trainer and a corpus author get from the oracle families: one
layout, one harness, and a label that a fixture earned by running the
candidates. This area spans the README capability `oracle-corpus`.

## Executable oracle families

- Problem: none open as shipped; the limits below belong to the first outcome
  in this area.
- Who: model trainers loading labels; corpus authors adding a family.
- Promise: Every family under `data/python-v2/materials` is a `family.json`,
  a `gold`, and `miss_1` through `miss_5`. The harness maps each family to
  its fixture, runs every candidate, and exits 0 only when every gold exits 0
  and every near miss fails on an assertion rather than a syntax or
  collection error. A family prefix scopes the run.
- Limits today: families `08-asyncio-concurrency-limiter`,
  `09-pydantic-field-cross-validation`, and
  `10-security-timing-constant-auth` have no fixture; the harness reports
  that no test file matched, skips them, and still exits 0, so their
  candidates are unproven. Every shipped family is a library-usage pitfall
  rather than an algorithm, data structure, or design pattern.
- Non-goals: a Docker, Terraform, or Git family (ROADMAP
  `platform-pilot-families`); restoring the legacy ranker corpora (ROADMAP
  `legacy-ranker-lineage`).
- Neighbours: none; first section of the area.
- Status rows: `oracle-corpus`.

## Oracle fixtures complete

- Problem: Three families ship candidates that no fixture has ever run, and
  the harness treats a missing fixture as a skip, so the green exit hides
  them.
- Who: model trainers, who would otherwise load three unproven labels.
- Promise: Every family has a fixture, the harness reports no skipped
  family, and a family without a fixture makes the harness exit non-zero.
- Non-goals: new families; changing the family layout.
- Open: whether a fixture-less family should fail the whole run or only be
  excluded from the count with a non-zero exit; either way the exit is
  non-zero.
- Neighbours: [Executable oracle families](#executable-oracle-families)
  above, whose Limits this removes.
- Outcome: `oracle-fixtures-complete`. Tracking: Not assigned.

## Algorithm, data structure, and design pattern families

- Problem: The `som-code-*` projects exist to teach the model algorithms,
  data structures, and design patterns, and no shipped family does; the
  discriminator can only learn to spot a misused library call.
- Who: model trainers; generator authors, whose L3 optimizer has to prefer
  the right structure, not only the right call.
- Promise: The corpus gains families whose gold and near misses differ in
  the algorithm, the data structure, or the design pattern used, each judged
  by a fixture that fails the near miss on behaviour, and each `family.json`
  names what the family teaches.
- Non-goals: a new layout; a benchmark suite from another project copied in
  without a fixture of its own.
- Open: which families come first; whether a family may share a fixture
  with a library-usage family that exercises the same behaviour; and whether
  the near misses for a design-pattern family are wrong patterns or broken
  implementations of the right one.
- Neighbours: [Oracle fixtures complete](#oracle-fixtures-complete) above,
  which fixes the harness these families are judged by; the decompiler
  outcome in [data-pipeline.md](data-pipeline.md), which will record which
  snippets each gold used.
- Outcome: `algorithm-data-structure-and-design-pattern-families`. Tracking: Not assigned.

## Non-goals in this area

- No family for a platform without a Python oracle; see ROADMAP
  `platform-pilot-families`.
- No promise about the legacy corpora under `data/som/` or
  `data/python-v1-rejected/`; see ROADMAP `legacy-ranker-lineage`.
