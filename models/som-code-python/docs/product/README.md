# SOM code corpus for Python product requirements

This corpus project owns the material the Snippet-Oriented Model learns
Python from: executable oracle families whose gold is the correct,
idiomatic program the generator must rebuild and whose near misses are
negative examples, the layer records that decompose each gold, and the
snippet instruction set the generator assembles programs from. This
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
reference document. This project teaches the Python ecosystem written as
modern, PEP 8, pythonic code that ruff, mypy, and pylint accept; algorithms,
data structures, and design patterns as such belong to the language-neutral
`som-code` corpus.

Boundaries that every section inherits:

- Every candidate is executed only by its fixture, under the harness, during
  verification; nothing here runs at inference.
- A family without a fixture proves nothing, whatever the harness exit code
  says; STATUS records it as unproven.
- A gold candidate that the quality gate rejects is a defect, whatever its
  fixture says.

## Who the corpus is for

| Reader | What they hold the corpus to |
|---|---|
| Model trainer | Every family the engine loads has a gold that passes and near misses that fail on behaviour, so a label is never a guess, and every record says why it is in the corpus. |
| Corpus author | One layout, one harness, one verifier; a new family or snippet is accepted or refused by a command, not by review. |
| Generator author | Every gold reassembles from its decompiled records, and the coverage figure says how much of it the ISA expresses as snippets. |

## Horizons

| Horizon | Outcome | Section |
|---|---|---|
| H1 | `oracle-fixtures-complete` | [corpus.md](corpus.md) § Oracle fixtures complete |
| H2 | `snippet-long-tail` | [snippet-isa.md](snippet-isa.md) § Snippet long tail |
| H2 | `non-minimal-near-misses` | [corpus.md](corpus.md) § Non-minimal near misses |

H1 is the data the engine cannot train without: the three unproven
families. H2 covers the ISA's long tail so more of each gold is a snippet rather than a
literal block, and hardens the near misses before the engine builds
preference data from them.

## Section index

| Section | File | Kind | Owner |
|---|---|---|---|
| Executable oracle families | corpus.md | shipped, limited | STATUS `oracle-corpus` |
| Curated rationale | corpus.md | shipped, limited | STATUS `curated-rationale` |
| Code quality | corpus.md | shipped | STATUS `code-quality` |
| Oracle fixtures complete | corpus.md | outcome | ROADMAP `oracle-fixtures-complete` |
| Non-minimal near misses | corpus.md | outcome | ROADMAP `non-minimal-near-misses` |
| Snippet ISA | snippet-isa.md | shipped, limited | STATUS `snippet-isa` |
| Snippet long tail | snippet-isa.md | outcome | ROADMAP `snippet-long-tail` |
| Project test suite | data-pipeline.md | shipped | STATUS `python-test-suite` |
| Decompiled layer records | data-pipeline.md | shipped, limited | STATUS `decompiled-layer-records` |
| uv-runnable packaging | data-pipeline.md | shipped | STATUS `uv-runnable-packaging` |

Non-goals are not sections. Each file ends with the non-goals that a reader of
that area would otherwise assume, pointing at the ROADMAP entry that gives the
reason: `algorithms-data-structures-and-design-patterns`, `platform-pilot-families`.
