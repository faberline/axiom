# Generation

What a generator user gets once the four layers exist: an intent becomes a
plan, the plan becomes a topology, the topology becomes an operation list,
and the operation list becomes files. None of it is shipped; every section
here is a future promise ordered top down. The layer design is
[docs/reference/architecture.md](../reference/architecture.md).

## L1 Planner

- Problem: There is no module that reads an intent, so nothing bounds the
  scope of a request before the lower layers would spend work on it.
- Who: generator users; the L2 layer, which consumes the plan.
- Promise: Given an intent, the planner emits the L1 record with intent,
  target, and constraints, and refuses a scope over five files or twenty
  blocks with `SCOPE_TOO_LARGE` before any lower layer runs.
- Non-goals: choosing files or snippets; the planner names what must exist,
  not how.
- Open: whether the planner is a trained model over decompiled L1 records or
  a rule set with the trained part below it.
- Neighbours: none; first section of the area. The record schema is the one
  the decompiler emits, owned by the discriminator area's first outcome.
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

- Problem: Nothing selects which snippet operation, with which parameters,
  fills a block, and the discriminator that would rank the choices is not
  wired to any layer.
- Who: the L4 assembler; model trainers judging the discriminator's ranking.
- Promise: Given an L2 record, the optimizer emits the ordered snippet
  operations with parameters, and the discriminator ranks the gold operation
  list above every near miss for a held-out family.
- Non-goals: writing files; the ISA content, which stays in the corpus
  project.
- Open: whether near-miss operation lists come from the decompiled near-miss
  candidates or from perturbing the gold list.
- Neighbours: [L2 Component Topology](#l2-component-topology) above; the
  discriminator area, whose backbone outcome this ranking depends on.
- Outcome: `l3-query-optimizer`. Tracking: Not assigned.

## L4 Rust Assembler

- Problem: The snippet ISA has a verifier in the corpus project but no
  executor; an operation list cannot become files.
- Who: generator users; corpus authors, whose ISA the assembler loads.
- Promise: An assembler executes an L3 operation list against the snippet ISA
  and writes files that compile and pass the family's oracle fixture.
- Non-goals: ISA content; running the produced program outside the fixture.
- Open: whether the assembler ships as a Rust binary under this project or as
  a library the CLI calls; the reference document names Rust, the roadmap
  keeps the choice open until the ISA loader is written.
- Neighbours: [L3 Query Optimizer](#l3-query-optimizer) above, whose
  operation list this executes.
- Outcome: `l4-rust-assembler`. Tracking: Not assigned.

## Non-goals in this area

- The assembler never runs the program it wrote at inference; see ROADMAP
  `run-user-code-at-inference`.
- No layer model is published as a weight file; see ROADMAP
  `published-model-artifact`.
