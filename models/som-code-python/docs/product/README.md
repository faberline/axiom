# SOM code corpus for Python product requirements

This corpus project owns the material the Snippet-Oriented Model learns
Python from: executable oracle families that teach the model to tell a
working algorithm, data structure, or design pattern from a near miss, and
the snippet instruction set the generator assembles programs from. This
directory is the product requirements document: what the corpus promises,
written down before the work items that deliver it. Release Milestones are
carved from these sections, not the other way round.

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

The corpus makes promises about data, never about a model. A family is
proven by its fixture running the candidates; a snippet is proven by the
verifier assembling and compiling it. The engine that trains on this data is
`models/som-core`, and the layer model it implements is that project's
reference document. The `som-code-*` projects exist so the model learns
algorithms, data structures, and design patterns from verified code; the
families shipped so far are library-usage pitfalls, and closing that gap is
the second outcome below.

Boundaries that every section inherits:

- Every candidate is executed only by its fixture, under the harness, during
  verification; nothing here runs at inference.
- A family without a fixture proves nothing, whatever the harness exit code
  says; STATUS records it as unproven.
- The legacy ranker lineage carried in this tree makes no promise and is a
  non-goal, not a section.

## Who the corpus is for

| Reader | What they hold the corpus to |
|---|---|
| Model trainer | Every family the engine loads has a gold that passes and near misses that fail on behaviour, so a label is never a guess. |
| Corpus author | One layout, one harness, one verifier; a new family or snippet is accepted or refused by a command, not by review. |
| Generator author | The ISA is complete enough to express the gold of every family it claims, and the decompiled records name which instructions the gold used. |

## Horizons

| Horizon | Outcome | Section |
|---|---|---|
| H1 | `oracle-fixtures-complete` | [corpus.md](corpus.md) § Oracle fixtures complete |
| H1 | `algorithm-data-structure-and-design-pattern-families` | [corpus.md](corpus.md) § Algorithm, data structure, and design pattern families |
| H1 | `decompiler-dsl-corpus` | [data-pipeline.md](data-pipeline.md) § Decompiler DSL corpus |
| H2 | `uv-runnable-packaging` | [data-pipeline.md](data-pipeline.md) § uv-runnable packaging |
| H2 | `legacy-suite-retired` | [data-pipeline.md](data-pipeline.md) § Legacy suite retired |
| H3 | `snippet-isa-v2` | [snippet-isa.md](snippet-isa.md) § Snippet ISA v2 |

H1 is the data the engine cannot train without: the three unproven families,
the families the project exists to teach, and the layer records the
generator learns from. H2 is hygiene that unblocks tooling. H3 widens the ISA
once an assembler exists to consume it.

## Section index

| Section | File | Kind | Owner |
|---|---|---|---|
| Executable oracle families | corpus.md | shipped, limited | STATUS `oracle-corpus` |
| Oracle fixtures complete | corpus.md | outcome | ROADMAP `oracle-fixtures-complete` |
| Algorithm, data structure, and design pattern families | corpus.md | outcome | ROADMAP `algorithm-data-structure-and-design-pattern-families` |
| Snippet ISA v1 | snippet-isa.md | shipped | STATUS `snippet-isa` |
| Snippet ISA v2 | snippet-isa.md | outcome | ROADMAP `snippet-isa-v2` |
| Project test suite | data-pipeline.md | shipped, limited | STATUS `python-test-suite` |
| Decompiler DSL corpus | data-pipeline.md | outcome | ROADMAP `decompiler-dsl-corpus` |
| uv-runnable packaging | data-pipeline.md | outcome | ROADMAP `uv-runnable-packaging` |
| Legacy suite retired | data-pipeline.md | outcome | ROADMAP `legacy-suite-retired` |

Non-goals are not sections. Each file ends with the non-goals that a reader of
that area would otherwise assume, pointing at the ROADMAP entry that gives the
reason: `legacy-ranker-lineage`, `platform-pilot-families`.
