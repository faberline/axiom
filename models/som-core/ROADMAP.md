# SOM core roadmap

## Purpose

This roadmap orders the work that turns the engine from one discriminator head
into the four-layer decompression generator the [README](README.md) describes.
Data comes first, then a discriminator worth trusting, then the layers from L1
down to L4, each an outcome with completion evidence the repository can run.
The current support state is in [STATUS.md](STATUS.md).

## Near-term outcomes

### Decompiler DSL corpus

- ID: `decompiler-dsl-corpus`
- Outcome: The engine loads the L1, L2, and L3 records that the corpus
  project's decompiler emits for every oracle family, so the layer models have
  training rows instead of only candidate rankings.
- Boundary: The decompiler itself, its output format, and its lossless
  reassembly proof belong to `models/som-code-python`; this outcome covers the
  loader in `src/som_core/dataset.py` and the schema it validates.
- Completion evidence: A `som train` run over the decompiled corpus reports
  the number of L1, L2, and L3 rows it loaded, and a loader test refuses a
  record missing any of the three layers.
- Tracking: Not assigned.

### Discriminator on a real backbone

- ID: `discriminator-on-a-real-backbone`
- Outcome: `som train` loads the base model named in a committed
  `sources.lock.json`, shuffles candidate positions per sample, and reports a
  validation accuracy that starts near chance and rises with training.
- Boundary: The repository root in `src/som_core/paths.py`, the lock file and
  its download, the dataset shuffle, the `--skeleton` default, and a
  shuffled-position validation control; not a new model architecture and not
  a published artifact.
- Completion evidence: A training summary whose initial validation accuracy
  is below the gold-first 100 percent that STATUS records today, produced
  with a base model whose digest matches the lock file.
- Tracking: Not assigned.

### L1 Planner

- ID: `l1-planner`
- Outcome: Given an intent, the planner emits the L1 record (intent, target,
  constraints) and refuses a scope over five files or twenty blocks with
  `SCOPE_TOO_LARGE` before any lower layer runs.
- Boundary: The planner module, its record schema, and the gatekeeper; not
  the topology or the snippet choice.
- Completion evidence: A test feeds an oversized intent and observes the
  refusal, and a second test feeds an in-scope intent and observes a record
  that validates against the schema the decompiler emits.
- Tracking: Not assigned.

### L2 Component Topology

- ID: `l2-component-topology`
- Outcome: Given an L1 record, the topology layer emits the files and blocks
  with their dependency order, matching the decompiled L2 record for every
  oracle family it was trained on.
- Boundary: The topology module and its schema; not the snippet operations
  inside a block.
- Completion evidence: A test compares the emitted topology with the
  decompiled L2 record for a held-out family and reports the block-level
  match.
- Tracking: Not assigned.

### L3 Query Optimizer

- ID: `l3-query-optimizer`
- Outcome: Given an L2 record, the optimizer emits the ordered snippet
  operations with parameters, and the discriminator ranks the gold operation
  list above every near miss for a held-out family.
- Boundary: The optimizer module, the operation vocabulary, and the
  discriminator wiring; not the assembly of files.
- Completion evidence: A test observes the operation list for a held-out
  family and the discriminator score ordering on it.
- Tracking: Not assigned.

### L4 Rust Assembler

- ID: `l4-rust-assembler`
- Outcome: An assembler executes an L3 operation list against the snippet ISA
  and writes files that compile and pass the family's oracle fixture.
- Boundary: The assembler binary and its ISA loader; not the ISA content,
  which stays in the corpus project.
- Completion evidence: A test assembles one family from its decompiled
  records, runs the fixture, and observes exit 0.
- Tracking: Not assigned.

## Later outcomes

### Rust target corpus

- ID: `rust-target-corpus`
- Outcome: A `models/som-code-rust` corpus project exists with its own oracle
  families and snippet ISA, and the engine trains on both languages from one
  command.
- Boundary: A second corpus project and the engine's `--domain` switch; not a
  cross-language planner.
- Completion evidence: `som train --domain rust` loads a non-empty Rust corpus
  and writes a training summary.
- Tracking: Not assigned.

## Non-goals

### Run user code at inference

- ID: `run-user-code-at-inference`
- Reason: The oracle fixtures execute candidates during corpus verification
  and training data construction only; the generator scores and assembles,
  it never executes what it produced or what a user typed.

### Published model artifact

- ID: `published-model-artifact`
- Reason: Checkpoints under `runs/` are local run output and stay ignored;
  publishing a weight file is a release decision for a later roadmap, not
  something the engine promises.
