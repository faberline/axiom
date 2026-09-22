# python-v2: the executable oracle corpus

This directory is training data, committed so that anyone who clones the
repository can train without a download step. It is not a raw dump. Every
family carries, in its own `family.json`, the reason it is in the corpus,
the reason its oracle has to be executable, and for each near miss the
observable consequence of its defect and the fixture tests that catch it.
Read this file first, then any `materials/<nn>-<name>/family.json`.

## Why executable oracles

SOM is a decompression code generator: L1 plans a program, L2 lays out its
components, L3 optimizes the queries between them, L4 assembles Rust or
Python from snippet instructions. A generator that only learns from correct
code learns what code looks like, not which nearby program is wrong. The
discriminator and preference (DPO) stages need pairs of a correct program
and a wrong one that is hard to tell apart by reading.

The corpus therefore judges candidates by running them. Each family has one
gold candidate that passes a pytest fixture and five near misses that fail
it on an assertion, never on a syntax or import error. The fixture is the
ground truth; prose is not. Static review cannot distinguish `<` from `<=`
at a boundary, a session that is closed from one that is dropped, or a
timestamp IV from a random one. The fixture can, and
`fixtures/verify_harness.py` proves it does for every candidate on every
run.

## Why one gold and five near misses

A single wrong program teaches the model where one line goes wrong. Five,
each with a different failure mode, teach that the same requirement fails in
different places: a default, a branch, an API choice, a boundary, a missing
check, or a missing cleanup. The near misses are minimal edits of the gold
candidate, so the diff between any pair is the lesson and nothing else.
Comments and docstrings are identical across the six candidates of a
family, so the only difference a model can learn from is behavior.

## Failure modes

| Failure mode | Near misses | What it teaches |
|---|---|---|
| `missing_validation` | 100 | A guard the requirement implies (empty input, range, type, containment) is absent, so bad input reaches the library instead of raising early. |
| `wrong_branch` | 95 | A condition is inverted, widened, or narrowed, so the code takes the other path on exactly the input the fixture sends. |
| `wrong_api_call` | 94 | A neighbouring API is used: the sync method for the async one, MD5 for PBKDF2, `close` for `release`, the defining module for the importing one. |
| `wrong_boundary` | 85 | An off-by-one at a limit: `<` for `<=`, `range(n)` for `range(n + 1)`, a size cap that rejects the documented maximum. |
| `wrong_default` | 81 | A default argument or constant that makes the unconfigured path wrong: retries of 0, a 1,000-iteration KDF, metrics off. |
| `missing_cleanup` | 47 | A `finally`, `close`, `release`, or `clear` is skipped or placed after a `yield`, so the failure only shows in the next test or the next request. |
| `shared_default_factory` | 3 | A mutable default or a module-level container is shared across calls, so one caller's state appears in another's. |

## What the families cover

101 families across 35 areas. The numbering is chronological, not
thematic; the area is in each `family.json`.

| Area group | Families | Areas |
|---|---|---|
| Web and persistence | 00 to 20 | fastapi, sqlalchemy, pydantic, asyncio |
| Security | 10, 71 to 80 | password hashing, PBKDF2, AES-GCM, CBC IVs, JWT algorithm, audience, expiry, claim leakage, CSPRNG tokens, PRNG isolation |
| Standard library and syntax | 21 to 35 | stdlib idioms, syntax pitfalls |
| Clients and messaging | 36 to 40 | requests, celery, redis |
| Numerics and ML | 41 to 50 | pandas, broadcasting, memory mutation, dtype casting, scikit-learn, pytorch |
| Process and filesystem | 51 to 57 | subprocess, yaml, shutil, os |
| Command-line | 58 to 60 | typer, click |
| HTTP clients and parsing | 61 to 67 | aiohttp, lxml, bs4 |
| Browser automation | 68 to 70 | playwright, selenium |
| Test discipline | 81 to 90 | pytest, unittest.mock, hypothesis |
| LLM and agent frameworks | 91 to 100 | openai, anthropic, fastmcp, langchain, langgraph |

## Record schema

`materials/<nn>-<name>/family.json`:

| Key | Meaning |
|---|---|
| `family_id`, `domain`, `area`, `capability` | Identity and taxonomy. |
| `requirement` | The task as a user would state it; the skeleton the candidates share. |
| `skeleton` | The shape every candidate keeps, so the near misses differ only in behavior. |
| `rationale.teaches` | One sentence naming the contract details the gold candidate gets right. |
| `rationale.why` | Why an executable oracle is needed here: what is invisible statically and what the fixture observes. |
| `oracle` | `fixtures/test_<nn>_<name>.py`, or `null` for the three families without a fixture yet. |
| `candidates[]` | `gold` plus `miss_1` to `miss_5`; each has `id`, `module`, `kind`. |
| `candidates[].failure_mode` | Near misses only; one of the seven modes above. |
| `candidates[].why_wrong` | Near misses only; the observable consequence of the defect, one sentence. |
| `candidates[].caught_by` | Near misses only; the sorted fixture tests that fail on this candidate, measured by running it. `null` when `oracle` is `null`. |
| `source_hashes` | Present on a few families; sha256 per candidate file. |

`caught_by` is measured, not written. The harness re-measures it on every
run and fails when the declared set and the failing set differ, so the
rationale cannot drift from the behavior it describes.

## How the layers consume it

- The discriminator head in `models/som-core` reads `candidates[].kind`
  and the candidate text and learns to rank gold above each near miss.
  `why_wrong` and `caught_by` are the supervision for the next step, a
  critic that names the defect and the test that would expose it.
- L1 (planner) reads `requirement` and `skeleton` as the target of a plan.
- L4 (assembler) reads the gold candidate as the reference realization of
  the snippet ISA under `../snippets/v1/`; the near misses are its
  hard negatives.
- L2 and L3 are not trained from this corpus yet; the roadmap in the
  project [ROADMAP](../../ROADMAP.md) names the corpus they need.

## Verification

From the repository root:

```
models/som-code-python/.venv/bin/python models/som-code-python/data/python-v2/fixtures/verify_harness.py
models/som-code-python/.venv/bin/python models/som-code-python/scripts/verify_curation.py
```

The harness runs every candidate against its fixture and requires gold exit
0, near miss exit 1, the declared `oracle` to be the fixture it ran, and the
declared `caught_by` to equal the tests that failed. The curation check
never executes a candidate; it requires the rationale fields to be present
and non-trivial, `caught_by` to name tests defined in the fixture, comments
and docstrings to be identical across the six candidates, and no label word
in any candidate source. Both print `[RESULT: SUCCESS]` or one line per
defect.

## Known gaps

- Families 08, 09, and 10 have no fixture; their `oracle` and `caught_by`
  are `null` and the harness skips them.
- The near misses are minimal edits of the gold candidate; a model that
  learns edit distance instead of behavior will do well here and badly on
  real code. Multi-defect and refactored near misses are the roadmap outcome
  `non-minimal-near-misses`.
- The families cover library-usage contracts. Algorithm, data-structure,
  and design-pattern families belong to `models/som-code` and are not here.

## Provenance

Every family was authored by hand for this corpus, in the shape that
`scripts/generator/github_miner_mock.py` describes. That script is a mock:
no family was mined from a GitHub issue, and no candidate is copied from a
third-party repository. The library APIs the candidates call are real; the
scenarios are synthetic.
