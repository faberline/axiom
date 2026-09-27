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
snippet ISA, the authored L1 plans, and the decompiler that measures L2 and
L3 records from each gold program; this project owns the record schema, the
models that learn from those records, and the assembler that executes an L3
operation list. The reference design of the four
layers is [docs/reference/architecture.md](../reference/architecture.md).

Boundaries that every section inherits:

- The generator selects and places verified snippets. It never emits library
  code that no snippet carries.
- Candidate execution happens in corpus verification and training data
  construction, never at inference.
- Every layer generates the next layer's record. No layer chooses among
  candidates a caller or the corpus supplies, and a near miss is training
  signal, never an inference-time option.
- A generated program is judged by its family's oracle fixture passing on
  the assembled files, not by an accuracy over a candidate list.

## Who the engine is for

| Reader | What they hold the engine to |
|---|---|
| Model trainer | One loader reads every corpus into weighted rows that carry the layer records, and a checkpoint is judged by the fixture pass rate of what it assembles. |
| Corpus author | One schema validates the decompiler's records, and the reference assembler turns a family's operation list back into its gold program. |
| Generator user | An in-scope intent becomes files that compile and pass the family fixture; an oversized intent is refused before any code exists. |

## Horizons

| Horizon | Outcome | Section |
|---|---|---|
| H1 | `layer-sft-on-a-real-backbone` | [training.md](training.md) § Layer SFT on a real backbone |
| H1 | `fixture-pass-rate-evaluation` | [training.md](training.md) § Fixture pass rate |
| H2 | `l1-planner` | [generation.md](generation.md) § L1 Planner |
| H2 | `l2-component-topology` | [generation.md](generation.md) § L2 Component Topology |
| H2 | `l3-query-optimizer` | [generation.md](generation.md) § L3 Query Optimizer |
| H2 | `l4-rust-assembler` | [generation.md](generation.md) § L4 Rust Assembler |
| H2 | `near-miss-preference-data` | [training.md](training.md) § Near-miss preference data |

H1 is a loop worth trusting before layers: next-token training on the layer
records, and an evaluation that runs the assembled program instead of
counting matches. H2 then follows the layer order top down, because each
layer consumes the record the one above it emits, and adds the near-miss
preference pass once SFT exists.

## Section index

| Section | File | Kind | Owner |
|---|---|---|---|
| Corpus loader | training.md | shipped, limited | STATUS `corpus-loader` |
| Layer records | training.md | shipped | STATUS `layer-records` |
| Layer SFT on a real backbone | training.md | outcome | ROADMAP `layer-sft-on-a-real-backbone` |
| Fixture pass rate | training.md | outcome | ROADMAP `fixture-pass-rate-evaluation` |
| Near-miss preference data | training.md | outcome | ROADMAP `near-miss-preference-data` |
| Reference assembler | generation.md | shipped | STATUS `reference-assembler` |
| L1 Planner | generation.md | outcome | ROADMAP `l1-planner` |
| L2 Component Topology | generation.md | outcome | ROADMAP `l2-component-topology` |
| L3 Query Optimizer | generation.md | outcome | ROADMAP `l3-query-optimizer` |
| L4 Rust Assembler | generation.md | outcome | ROADMAP `l4-rust-assembler` |

Non-goals are not sections. Each file ends with the non-goals that a reader of
that area would otherwise assume, pointing at the ROADMAP entry that gives the
reason: `candidate-selection-at-inference`, `run-user-code-at-inference`,
`published-model-artifact`.
