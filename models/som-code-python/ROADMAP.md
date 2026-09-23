# SOM code corpus for Python roadmap

## Purpose

This roadmap orders the work that turns the Python corpus into the training
material the four-layer generator in `models/som-core` needs. The order is
data first: finish the oracle fixtures, emit the decompiled layer records,
then harden the near misses and widen the ISA. The subject stays the Python
ecosystem written as code its community accepts. The current
support state is in [STATUS.md](STATUS.md).

## Near-term outcomes

### Oracle fixtures complete

- ID: `oracle-fixtures-complete`
- Outcome: Every family under `data/curated/families` has a fixture, the
  harness reports no skipped family, it exits non-zero when a family has no
  fixture instead of skipping it, and no `oracle` or `caught_by` field is
  `null`.
- Boundary: The three missing fixtures for `08-asyncio-concurrency-limiter`,
  `09-pydantic-field-cross-validation`, and
  `10-security-timing-constant-auth`, plus the harness's handling of a
  missing fixture; not new families.
- Completion evidence: The harness run over `08 09 10` shows each gold
  exiting 0 and each near miss failing on exactly its declared `caught_by`,
  the curation check reports no `null` field, and a run with one fixture
  renamed away exits non-zero.
- Tracking: Not assigned.

### Decompiler DSL corpus

- ID: `decompiler-dsl-corpus`
- Outcome: A decompiler turns every family into its L1 intent, L2 component
  topology, and L3 operation sequence records, distinguishes gold from near
  miss with the failure mode, and reassembles the multi-file TODO fixture
  from its records with no AST loss.
- Boundary: The decompiler script, its output files, and the tests that
  today sit at the repository root under `tests/data_pipeline/` and
  `tests/fixtures/multi_file_todo/`, which move into this project; the
  engine's loader is the engine's outcome.
- Completion evidence: The decompiler test module passes, including the
  case that processes every family with exit 0 and the AST-equivalence case
  over the TODO fixture.
- Tracking: Not assigned.

## Later outcomes

### Non-minimal near misses

- ID: `non-minimal-near-misses`
- Outcome: Families gain near misses that are not a one-line edit of the
  gold candidate, refactored or carrying two defects, so a discriminator that
  scores edit distance instead of behaviour stops scoring well on the corpus.
- Boundary: New near-miss candidates and their `family.json` entries under
  `data/curated/families`, each still failing its fixture on an assertion
  with a measured `caught_by`; not new families and not the harness.
- Completion evidence: The harness and the curation check exit 0 over the
  changed families, and `diff` between each new candidate and its gold shows
  more than one hunk.
- Tracking: Not assigned.

### Snippet ISA v2

- ID: `snippet-isa-v2`
- Outcome: The ISA covers the libraries the oracle families use beyond
  FastAPI, Pydantic, and SQLAlchemy, and every family's gold can be expressed
  as an operation list over it.
- Boundary: New snippet definitions under `data/snippets/v2/` and the
  verifier's required set; not the assembler.
- Completion evidence: The verifier run over `v2` compiles the assembled
  module, and a coverage report names no family whose gold uses a library the
  ISA lacks.
- Tracking: Not assigned.

## Non-goals

### Algorithms, data structures, and design patterns

- ID: `algorithms-data-structures-and-design-patterns`
- Reason: Families that differ in an algorithm, a data structure, or a design
  pattern as such are language-neutral and belong to the `som-code` corpus;
  this project teaches how the Python ecosystem is used and written, and a
  family here is judged by library behaviour and community style.

### Platform pilot families

- ID: `platform-pilot-families`
- Reason: Docker, Terraform, and Git scenarios have no executable oracle in
  Python, so a family for them cannot meet the corpus contract; they belong
  to a corpus project for that platform, if one is ever opened.
