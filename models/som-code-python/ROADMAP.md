# SOM code corpus for Python roadmap

## Purpose

This roadmap orders the work that turns the Python corpus into the training
material the four-layer generator in `models/som-core` needs. The order is
data first: finish the oracle fixtures, then harden the near misses and
cover the ISA's long tail so more of each gold is expressed as snippets. The subject stays the Python
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

## Later outcomes

### Non-minimal near misses

- ID: `non-minimal-near-misses`
- Outcome: Families gain near misses that are not a one-line edit of the
  gold candidate, refactored or carrying two defects, so preference data
  built from them cannot be separated from the gold by edit distance alone.
- Boundary: New near-miss candidates and their `family.json` entries under
  `data/curated/families`, each still failing its fixture on an assertion
  with a measured `caught_by`; not new families and not the harness.
- Completion evidence: The harness and the curation check exit 0 over the
  changed families, and `diff` between each new candidate and its gold shows
  more than one hunk.
- Tracking: Not assigned.

### Snippet long tail

- ID: `snippet-long-tail`
- Outcome: Route handlers with branching bodies, exception handlers, CLI
  commands, and async code become snippets where at least two families share
  the idiom, so the verifier's coverage rises without a template line that is
  only a placeholder.
- Boundary: New snippet definitions under `data/snippets/<library>/` and new
  families that give a one-family idiom its second use; not the template
  syntax and not the assembler.
- Completion evidence: `scripts/verify_snippets.py` exits 0 with a higher
  `coverage:` line than STATUS `snippet-isa` records, and
  `scripts/verify_roundtrip.py` still rebuilds every family it did before.
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
