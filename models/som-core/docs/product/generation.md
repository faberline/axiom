# Generation

What a generator user gets once the four layers exist: an intent becomes a
plan, the plan becomes a topology, the topology becomes an operation list,
and the operation list becomes files. Each layer generates its record; none
chooses from candidates. The Python reference assembler is shipped; the
layer models are future promises ordered top down, and their record schema
ships in the training area. The layer design is
[docs/reference/architecture.md](../reference/architecture.md).

## Reference assembler

- Problem: none open as shipped.
- Who: corpus authors proving a decompiled family reassembles; the fixture
  pass rate evaluation, which assembles every generated operation list.
- Promise: `som assemble` executes an L3 operation list, read from a JSON
  file or from a `family.json`'s `decompiled.ops`, and writes each file it
  creates under `--out`: the module docstring, then the imports in order,
  then each block, blocks separated by two blank lines. `INSERT_SNIPPET`
  renders the snippet's template from the ISA directory with the operation's
  parameters and refuses a missing snippet or parameter; `INSERT_BLOCK`
  writes its literal source. The list is validated against the layer
  records schema before anything is written.
- Non-goals: formatting or linting the output; executing it.
- Neighbours: none; first section of the area. The corpus project's
  round-trip gate calls it for every family.
- Status rows: `reference-assembler`.

## L1 Planner

- Problem: There is no module that reads an intent, so nothing bounds the
  scope of a request before the lower layers would spend work on it.
- Who: generator users; the L2 layer, which consumes the plan.
- Promise: Given an intent, the planner emits the L1 record with intent,
  target, and constraints, and refuses a scope over five files or twenty
  blocks with `SCOPE_TOO_LARGE` before any lower layer runs.
- Non-goals: choosing files or snippets; the planner names what must exist,
  not how.
- Open: whether the planner is a model trained on the corpus's authored
  `plan` records or a rule set with the trained part below it.
- Neighbours: [Reference assembler](#reference-assembler) above, which
  runs last. The record schema is
  [layer-records.md](../reference/layer-records.md), which the training
  area's `layer-records` row enforces.
- Outcome: `l1-planner`. Tracking: Not assigned.

## L2 Component Topology

- Problem: Nothing turns a plan into files and blocks with a dependency
  order, so an assembler would have nowhere to put a snippet.
- Who: the L3 layer; corpus authors checking the topology against the
  decompiled record.
- Promise: Given an L1 record, the topology layer emits the files and blocks
  with their dependency order, and for a held-out oracle family the block
  layout matches the decompiled L2 record.
- Non-goals: the snippet operations inside a block.
- Open: how a block is identified across a rename; the decompiler's L2
  record fixes the identity rule.
- Neighbours: [L1 Planner](#l1-planner) above, whose record this consumes.
- Outcome: `l2-component-topology`. Tracking: Not assigned.

## L3 Query Optimizer

- Problem: Nothing emits which snippet operation, with which parameters,
  fills a block, so no topology becomes an operation list.
- Who: the L4 assembler; model trainers judging generated operation lists by
  the fixture pass rate.
- Promise: Given an L2 record, the optimizer generates the ordered operation
  list with parameters, and for a held-out family the list assembles into
  files that pass its oracle fixture.
- Non-goals: writing files; the ISA content, which stays in the corpus
  project; choosing among operation lists a caller supplies.
- Open: how far the optimizer may fall back to `INSERT_BLOCK` literal source
  when the ISA lacks a snippet, before the corpus project's ISA v2 closes the
  gap.
- Neighbours: [L2 Component Topology](#l2-component-topology) above; the
  training area, whose SFT and preference outcomes train this layer.
- Outcome: `l3-query-optimizer`. Tracking: Not assigned.

## L4 Rust Assembler

- Problem: The reference assembler is Python, and the design names a Rust
  executor for production speed; nothing holds a second implementation to
  the first.
- Who: generator users; corpus authors, whose ISA the assembler loads.
- Promise: A Rust assembler executes an L3 operation list against the
  snippet ISA and writes files byte-identical to what `som assemble` writes
  for the same list, for every family in the corpus.
- Non-goals: ISA content; running the produced program outside the fixture.
- Open: whether the Rust assembler ships as a binary the CLI shells out to
  or as a Python extension.
- Neighbours: [L3 Query Optimizer](#l3-query-optimizer) above, whose
  operation list this executes.
- Outcome: `l4-rust-assembler`. Tracking: Not assigned.

## Non-goals in this area

- The assembler never runs the program it wrote at inference; see ROADMAP
  `run-user-code-at-inference`.
- No layer model is published as a weight file; see ROADMAP
  `published-model-artifact`.
