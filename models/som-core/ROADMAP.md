# SOM core roadmap

## Purpose

This roadmap orders the work that turns the engine's record schema and
reference assembler into the four-layer decompression generator the
[README](README.md) describes. Layer SFT and an evaluation that runs what the
layers produce come first, then the layers from L1 down to L4, each an
outcome with completion evidence the repository can run. No outcome trains a
model to choose among candidates.
The current support state is in [STATUS.md](STATUS.md).

## Near-term outcomes

### Layer SFT on a real backbone

- ID: `layer-sft-on-a-real-backbone`
- Outcome: `som train` loads the base model a committed `sources.lock.json`
  names and fine-tunes it with LoRA and next-token loss on three pair sets
  built from the loaded rows (caption to L1 plan, L1 plan to L2 topology, L2
  topology to L3 operation list), sampling rows by layer weight.
- Boundary: The lock file and its download, the repository root in
  `src/som_core/paths.py` (one directory short today), the pair builder, and
  the training loop; not a new architecture and not a published artifact.
- Completion evidence: A training summary with per-layer, per-pair
  validation loss that falls across the run, produced with a base model
  whose digest matches the lock file.
- Tracking: Not assigned.

### Fixture pass rate evaluation

- ID: `fixture-pass-rate-evaluation`
- Outcome: `som eval` generates L1, L2, and L3 for every held-out family from
  its caption, assembles the operation list, runs the family's oracle
  fixture on the result, and reports the share of families whose fixture
  exits 0.
- Boundary: The evaluation command and its report; not a score against near
  misses and not execution outside a fixture.
- Completion evidence: A report over the held-out split naming each family's
  fixture exit, where a checkpoint that emits the gold records scores every
  fixtured family and an empty checkpoint scores none.
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
- Outcome: Given an L1 record, the topology layer generates the files and
  blocks with their dependency order, and the records assemble into a
  program that passes the family's fixture.
- Boundary: The topology module and its schema; not the snippet operations
  inside a block.
- Completion evidence: A test compares the emitted topology with the
  decompiled L2 record for a held-out family and reports the block-level
  match.
- Tracking: Not assigned.

### L3 Query Optimizer

- ID: `l3-query-optimizer`
- Outcome: Given an L2 record, the optimizer generates the ordered
  operation list with parameters, and the reference assembler turns it into
  files that pass the family's fixture for a held-out family.
- Boundary: The optimizer model; not the operation vocabulary, which
  docs/reference/layer-records.md fixes, and not the assembly of files.
- Completion evidence: `som eval` reports the held-out family's fixture
  exit 0 on the assembled output of a generated operation list.
- Tracking: Not assigned.

### L4 Rust Assembler

- ID: `l4-rust-assembler`
- Outcome: A Rust assembler executes an L3 operation list against the
  snippet ISA and writes files byte-identical to `som assemble`.
- Boundary: The assembler binary and its ISA loader; not the ISA content,
  which stays in the corpus project, and not the Python reference assembler.
- Completion evidence: A test assembles every curated family with both
  assemblers and observes identical bytes.
- Tracking: Not assigned.

## Later outcomes

### Near-miss preference data

- ID: `near-miss-preference-data`
- Outcome: Each near miss is decompiled to its own L3 operation list and a
  preference pass (DPO) trains the optimizer on gold versus near-miss lists
  for the same L2 topology.
- Boundary: The near-miss decompilation and the preference pass; not near
  misses offered as options at inference.
- Completion evidence: A run whose summary reports the preference margin on
  held-out pairs and a fixture pass rate no lower than SFT alone.
- Tracking: Not assigned.

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
  and training data construction only; the generator plans and assembles,
  it never executes what it produced or what a user typed.

### Candidate selection at inference

- ID: `candidate-selection-at-inference`
- Reason: SOM generates each layer's record; a model that picks one of a
  caller's candidate programs is the JEV selector shape SOM replaced, and
  near misses serve only as negative training data.

### Published model artifact

- ID: `published-model-artifact`
- Reason: Checkpoints under `runs/` are local run output and stay ignored;
  publishing a weight file is a release decision for a later roadmap, not
  something the engine promises.
