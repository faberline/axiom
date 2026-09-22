# SOM code corpus for Python roadmap

## Purpose

This roadmap orders the work that turns the Python corpus into the training
material the four-layer generator in `models/som-core` needs. The order is
data first: finish the oracle fixtures, add the algorithm, data-structure, and
design-pattern families the project exists to teach, emit the decompiled
layer records, then fix packaging and prune the legacy tests. The current
support state is in [STATUS.md](STATUS.md).

## Near-term outcomes

### Oracle fixtures complete

- ID: `oracle-fixtures-complete`
- Outcome: Every family under `data/python-v2/materials` has a fixture, the
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

### Algorithm, data structure, and design pattern families

- ID: `algorithm-data-structure-and-design-pattern-families`
- Outcome: The corpus gains families whose gold and near misses differ in an
  algorithm, a data structure, or a design pattern, each with a fixture that
  fails the near miss on behaviour, so the discriminator learns those
  distinctions and not only library-usage pitfalls.
- Boundary: New families and fixtures under `data/python-v2` following the
  existing layout; not a new layout, and not the engine.
- Completion evidence: The harness run over the new families shows every
  gold exiting 0 and every near miss failing on an assertion, and
  `family.json` for each names the algorithm, structure, or pattern it
  teaches.
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

### uv-runnable packaging

- ID: `uv-runnable-packaging`
- Outcome: `uv run --project models/som-code-python` resolves `som-core` from
  its sibling path and imports the `som` package from where it lives.
- Boundary: `pyproject.toml` and the lock file only; no code moves.
- Completion evidence: `uv run --project models/som-code-python python -c
  "import som, som_core"` exits 0 from the repository root.
- Tracking: Not assigned.

### Legacy suite retired

- ID: `legacy-suite-retired`
- Outcome: The project test suite exits 0 because every test that reads a
  legacy corpus, runtime, or lock file that is not tracked here is removed
  or moved with the data it needs.
- Boundary: The tests named in the STATUS row `python-test-suite` and the
  modules only they exercise; not the oracle, candidate quality, corpus
  materializer, seed inventory, training runtime, or snippet tests.
- Completion evidence: The pytest gate in STATUS exits 0 with no
  deselection.
- Tracking: Not assigned.

## Later outcomes

### Non-minimal near misses

- ID: `non-minimal-near-misses`
- Outcome: Families gain near misses that are not a one-line edit of the
  gold candidate, refactored or carrying two defects, so a discriminator that
  scores edit distance instead of behaviour stops scoring well on the corpus.
- Boundary: New near-miss candidates and their `family.json` entries under
  `data/python-v2/materials`, each still failing its fixture on an assertion
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

### Legacy ranker lineage

- ID: `legacy-ranker-lineage`
- Reason: The specialist and developer ranker research, its runtimes, and
  the `som-research-*` lock files predate the layer model and are kept only
  so that history can be read; the v4 seed adapter they expect stays local
  and untracked. Nothing here promises to restore their data or keep their
  tests green.

### Platform pilot families

- ID: `platform-pilot-families`
- Reason: Docker, Terraform, and Git scenarios have no executable oracle in
  Python, so a family for them cannot meet the corpus contract; they belong
  to a corpus project for that platform, if one is ever opened.
