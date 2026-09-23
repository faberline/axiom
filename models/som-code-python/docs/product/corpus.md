# Corpus

What a model trainer and a corpus author get from the oracle families: one
layout, one harness, a label that a fixture earned by running the
candidates, and the written reason each record is in the corpus. This area
spans the README capabilities `oracle-corpus`, `curated-rationale`, and
`code-quality`.

## Executable oracle families

- Problem: none open as shipped; the limits below belong to the first outcome
  in this area.
- Who: model trainers loading labels; corpus authors adding a family.
- Promise: Every family under `data/curated/families` is a `family.json`,
  a `gold`, and `miss_1` through `miss_5`. The harness maps each family to
  its fixture, runs every candidate, and exits 0 only when every gold exits 0
  and every near miss fails on an assertion rather than a syntax or
  collection error, on exactly the tests its `caught_by` declares, with the
  declared `oracle` being the fixture that ran. A family prefix scopes the
  run.
- Limits today: families `08-asyncio-concurrency-limiter`,
  `09-pydantic-field-cross-validation`, and
  `10-security-timing-constant-auth` have no fixture; the harness reports
  that no test file matched, skips them, and still exits 0, so their
  candidates are unproven.
- Non-goals: a Docker, Terraform, or Git family (ROADMAP
  `platform-pilot-families`); a family about an algorithm, data structure,
  or design pattern as such (ROADMAP `algorithms-data-structures-and-design-patterns`).
- Neighbours: [Curated rationale](#curated-rationale) below, whose
  `caught_by` this harness measures.
- Status rows: `oracle-corpus`.

## Curated rationale

- Problem: none open as shipped; the limit below is removed by the outcome
  that follows.
- Who: model trainers who clone the corpus and need to know why a record is
  there before they train on it; critic authors who need the defect named
  and the test that exposes it.
- Promise: Every `family.json` states what the family teaches and why its
  oracle has to be executable, names the fixture the harness must run, and
  for every near miss states the observable consequence of its defect and
  the fixture tests that fail on it. `caught_by` is measured by running the
  candidate, and the harness fails when the declaration and the measurement
  differ. The curation check refuses a missing or trivial field, a
  consequence that only repeats the failure mode, a test the fixture does
  not define, a label word in candidate source, and a comment or docstring
  that differs across the six candidates. Every gold program also carries a
  `caption`, a long description that is the planner's prompt, and a
  `decompiled` block measured from its AST; the check refuses a caption
  outside 80 to 400 words, one that pastes the requirement, one that omits
  an exception the gold raises, a status code it declares, or a library it
  imports, and a `decompiled` block that no longer matches the source. The
  README under `data/curated`
  carries the reasoning for a reader who starts from the data.
- Limits today: families `08-asyncio-concurrency-limiter`,
  `09-pydantic-field-cross-validation`, and
  `10-security-timing-constant-auth` have no fixture, so their `oracle` and
  `caught_by` are `null` and their `why_wrong` sentences are unmeasured.
- Non-goals: a rationale generated from the diff (the sentence is authored,
  the test list is measured); a per-line annotation of the candidates.
- Neighbours: [Executable oracle families](#executable-oracle-families)
  above, whose harness measures `caught_by`; [Oracle fixtures
  complete](#oracle-fixtures-complete) below, which removes the limit.
- Status rows: `curated-rationale`.

## Code quality

- Problem: none open as shipped.
- Who: model trainers, whose generator imitates the gold programs, so a
  gold that the community's tools reject teaches code nobody would merge;
  corpus authors, who need style judged by a command rather than a review.
- Promise: Every gold candidate is modern, PEP 8, typed Python that passes
  ruff with the full selection, ruff format, mypy in strict mode, and pylint
  with zero messages, under the tool tables in `pyproject.toml`. Every near
  miss passes ruff format and the style rules `E,W,I,UP`, so style never
  separates it from its gold. The gate exits non-zero and names every
  candidate and message that fails.
- Non-goals: the behavioural lint rules on near misses, whose defects may be
  exactly what those rules flag; a type checker or linter the community does
  not run.
- Neighbours: [Curated rationale](#curated-rationale) above, whose check
  requires the six candidates to share every comment and docstring.
- Status rows: `code-quality`.

## Oracle fixtures complete

- Problem: Three families ship candidates that no fixture has ever run, and
  the harness treats a missing fixture as a skip, so the green exit hides
  them.
- Who: model trainers, who would otherwise load three unproven labels.
- Promise: Every family has a fixture, the harness reports no skipped
  family, a family without a fixture makes the harness exit non-zero, and no
  `oracle` or `caught_by` field is `null`.
- Non-goals: new families; changing the family layout.
- Open: whether a fixture-less family should fail the whole run or only be
  excluded from the count with a non-zero exit; either way the exit is
  non-zero.
- Neighbours: [Executable oracle families](#executable-oracle-families)
  and [Curated rationale](#curated-rationale) above, whose Limits this
  removes.
- Outcome: `oracle-fixtures-complete`. Tracking: Not assigned.

## Non-minimal near misses

- Problem: Every near miss is a one-line edit of its gold, so a model can
  score the corpus by edit distance without learning the behaviour the
  fixture judges.
- Who: model trainers, whose validation score would overstate the
  discriminator; critic authors, who need a defect that is not the only
  changed line.
- Promise: Families gain near misses that are refactored or carry two
  defects, each still failing its fixture on an assertion with a measured
  `caught_by`, and `diff` against the gold shows more than one hunk.
- Non-goals: new families; a change to the harness.
- Open: how many such candidates a family needs; whether they replace two
  of the five minimal ones or extend the set past six, which changes the
  layout the curation check enforces.
- Neighbours: [Curated rationale](#curated-rationale) above, whose check
  the new candidates must pass.
- Outcome: `non-minimal-near-misses`. Tracking: Not assigned.

## Non-goals in this area

- No family for a platform without a Python oracle; see ROADMAP
  `platform-pilot-families`.
- No family about an algorithm, data structure, or design pattern as such;
  see ROADMAP `algorithms-data-structures-and-design-patterns`.
