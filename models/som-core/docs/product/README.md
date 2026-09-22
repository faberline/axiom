# SOM core product requirements

SOM core is the engine of the Snippet-Oriented Model: a decompression code
generator that plans, lays out, optimises, and assembles a program from a
frozen snippet instruction set. This directory is the product requirements
document: what the engine promises per layer, written down before the work
items that deliver it. Release Milestones are carved from these sections, not
the other way round.

## How this directory is organised

- One file per capability area, named for the area and never for a work item.
  Each `## <title>` section is one promise.
- A shipped promise names the [STATUS](../../STATUS.md) rows that measure it.
  A future promise names the [ROADMAP](../../ROADMAP.md) outcome that owns it
  and ends with `Tracking: Not assigned.` until its release Milestone exists.
- A future section is written before its release Milestone. Use
  `aw-grill-release plan` to fix the promise, version, issue set, and order
  without writing. Its approved `apply` opens or updates the Milestone, adds
  ` (Milestone #<number>)` to the heading, and writes the typed issues.
- Every section carries the parts `aw-grill-release plan` asks for: Problem,
  Who, Promise, Non-goals, Neighbours, plus `Open:` lines for decisions the
  release plan still has to settle. An `Open:` line is a question, not a
  default.
- A new capability area is a change to this index first and to the README
  `### Capability index` when the area ships.

## Positioning

The engine consumes what the corpus projects produce and never owns corpus
content. `models/som-code-python` owns the executable oracle families, the
snippet ISA, and the decompiler that turns a family into L1, L2, and L3
records; this project owns the models that learn from those records and the
assembler that executes an L3 operation list. The reference design of the four
layers is [docs/reference/architecture.md](../reference/architecture.md).

Boundaries that every section inherits:

- The generator selects and places verified snippets. It never emits library
  code that no snippet carries.
- Candidate execution happens in corpus verification and training data
  construction, never at inference.
- One trained component exists today, the candidate discriminator; every
  layer below is a future promise until STATUS says otherwise.

## Who the engine is for

| Reader | What they hold the engine to |
|---|---|
| Model trainer | One command trains against a corpus directory, records what it loaded, and reports an accuracy that cannot be 100 percent before training. |
| Corpus author | The engine's loader refuses a record that does not match the decompiler's schema, so a corpus defect surfaces at load time. |
| Generator user | An in-scope intent becomes files that compile and pass the family fixture; an oversized intent is refused before any code exists. |

## Horizons

| Horizon | Outcome | Section |
|---|---|---|
| H1 | `decompiler-dsl-corpus` | [discriminator.md](discriminator.md) § Layer records from the decompiler |
| H1 | `discriminator-on-a-real-backbone` | [discriminator.md](discriminator.md) § Discriminator on a real backbone |
| H2 | `l1-planner` | [generation.md](generation.md) § L1 Planner |
| H2 | `l2-component-topology` | [generation.md](generation.md) § L2 Component Topology |
| H2 | `l3-query-optimizer` | [generation.md](generation.md) § L3 Query Optimizer |
| H2 | `l4-rust-assembler` | [generation.md](generation.md) § L4 Rust Assembler |

H1 is data and trust before layers: the decompiler records are the training
rows every layer needs, and a discriminator that scores 100 percent before
its first update is not evidence the optimizer can rank on. H2 then follows
the layer order top down, because each layer consumes the record the one
above it emits.

## Section index

| Section | File | Kind | Owner |
|---|---|---|---|
| Candidate discriminator training | discriminator.md | shipped, limited | STATUS `training-cli-surface`, `discriminator-training-cli` |
| Corpus loader | discriminator.md | shipped | STATUS `corpus-loader` |
| Layer records from the decompiler | discriminator.md | outcome | ROADMAP `decompiler-dsl-corpus` |
| Discriminator on a real backbone | discriminator.md | outcome | ROADMAP `discriminator-on-a-real-backbone` |
| L1 Planner | generation.md | outcome | ROADMAP `l1-planner` |
| L2 Component Topology | generation.md | outcome | ROADMAP `l2-component-topology` |
| L3 Query Optimizer | generation.md | outcome | ROADMAP `l3-query-optimizer` |
| L4 Rust Assembler | generation.md | outcome | ROADMAP `l4-rust-assembler` |

Non-goals are not sections. Each file ends with the non-goals that a reader of
that area would otherwise assume, pointing at the ROADMAP entry that gives the
reason: `run-user-code-at-inference`, `published-model-artifact`.
