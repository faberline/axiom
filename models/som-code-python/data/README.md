# The executable oracle corpus

This directory is training data, committed so that anyone who clones the
repository can train without a download step. It is not a raw dump. Every
family carries, in its own `family.json`, the reason it is in the corpus,
the reason its oracle has to be executable, and for each near miss the
observable consequence of its defect and the fixture tests that catch it.
Read this file first, then any `curated/families/<nn>-<name>/family.json`.

## Layout

| Path | Owner | What it holds |
|---|---|---|
| `curated/families/<nn>-<name>/` | this project | `family.json` plus one candidate file per entry; committed and gated. |
| `curated/fixtures/test_<nn>_<name>.py` | this project | The oracle each curated family names in `oracle`. |
| `user/families/`, `user/fixtures/` | you | Families you add to train on locally, in the same schema; never committed. See [user/README.md](user/README.md). |
| `snippets/<library>/<id>.json` | this project | The snippet ISA the assembler realizes. |

The two corpus layers are separate so that the curated set stays a
reviewed, reproducible baseline while anyone can train on more. They train
together, not one after the other: `som train` with no `--data-dir` loads
`curated`, then `user` when it holds a family, tags every row with its
layer, and samples each layer by its weight. A user family is a team's own
convention, so it outranks the baseline by default: `curated=1`, `user=2`,
meaning each user family appears twice per epoch for each curated one.
`--layer-weight user=3` (repeatable) or `SOM_LAYER_WEIGHTS=user=3,curated=1`
changes that; a weight of 0 drops a layer from training but keeps it in
validation. Validation is split per layer and reported per layer, so a run
shows whether the team's conventions were learned and whether the curated
baseline regressed.

A user family replaces a curated one only by saying so: `"overrides":
"som:python:<nn>-<name>"` in its `family.json`. The loader refuses an
`overrides` in the curated layer, a target that is not loaded, two user
families overriding the same target, and any undeclared id collision, so a
typo can never silently train both versions. There are no version
directories: a schema change migrates every family in place and the gates
prove it.

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
`scripts/verify_harness.py` proves it does for every candidate on every
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

`<layer>/families/<nn>-<name>/family.json`:

| Key | Meaning |
|---|---|
| `family_id`, `domain`, `area`, `capability` | Identity and taxonomy. |
| `requirement` | The task as a user would state it; the skeleton the candidates share. |
| `skeleton` | The shape every candidate keeps, so the near misses differ only in behavior. |
| `caption` | The long description of the gold program, 80 to 400 words, written the way an image model's training caption describes a picture: every component, its defaults, its guards, the exceptions it raises, the libraries it imports, and what the fixture asserts. It is the planner's input, written in the voice of a user who already knows exactly what they want. |
| `rationale.teaches` | One sentence naming the contract details the gold candidate gets right. |
| `rationale.why` | Why an executable oracle is needed here: what is invisible statically and what the fixture observes. |
| `oracle` | `fixtures/test_<nn>_<name>.py`, or `null` for the three families without a fixture yet. |
| `decompiled` | What the gold source says about itself, measured from its AST by `scripts/decompile_gold.py`: `surface` (top-level classes with bases and methods, functions with parameters and any HTTP route or CLI command, module constants), `imports` (third-party top-level modules), `raises` (exception classes raised by name), `status_codes`. |
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
- L1 (planner) reads `caption` as its prompt and `requirement` and
  `skeleton` as the short forms of the same target. A requirement is how a
  user asks; a caption is everything a user would have to say for one
  program to be the only right answer, the way image models are trained on
  long synthetic captions and then prompted with short ones.
- L2 (component topology) reads `decompiled.surface` as the component list
  the plan must lay out; `decompiled.imports` names its dependencies.
- L4 (assembler) reads the gold candidate as the reference realization of
  the snippet ISA under `snippets/`; the near misses are its
  hard negatives.
- L3 is not trained from this corpus yet; the roadmap in the
  project [ROADMAP](../ROADMAP.md) names the corpus they need.

## Verification

From the repository root:

```
uv run --project models/som-code-python python models/som-code-python/scripts/verify_harness.py
uv run --project models/som-code-python python models/som-code-python/scripts/verify_curation.py
uv run --project models/som-code-python python models/som-code-python/scripts/verify_quality.py
```

All three read `data/curated` by default; pass `--corpus models/som-code-python/data/user`
to hold your own families to the same bar.

The harness runs every candidate against its fixture and requires gold exit
0, near miss exit 1, the declared `oracle` to be the fixture it ran, and the
declared `caught_by` to equal the tests that failed. The curation check
never executes a candidate; it requires the rationale fields to be present
and non-trivial, `caught_by` to name tests defined in the fixture, comments
and docstrings to be identical across the six candidates, and no label word
in any candidate source. It also requires every `caption` to be 80 to 400
words, free of label words, not the `requirement` pasted in, and honest
about the gold source: it names each exception class the gold raises, each
status code it declares, and each third-party library it imports, and names
no exception class that neither the gold nor the fixture mentions. And it
re-measures `decompiled` from the gold source and fails on any difference,
so the stored facts cannot drift, the same discipline as `caught_by`. The
quality gate holds every gold candidate to ruff, ruff format, `mypy
--strict`, and pylint with zero messages, and every near miss to ruff format
and the style rules `E,W,I,UP`, so a model cannot tell gold from near miss by
style. All three print `[RESULT: SUCCESS]` or one line per defect.

`models/som-code-python/scripts/decompile_gold.py --write` refreshes
`decompiled` after a gold candidate changes; `--draft <dir>` writes one
sheet per family for the person revising its caption.

## Known gaps

- Families 08, 09, and 10 have no fixture; their `oracle` and `caught_by`
  are `null` and the harness skips them.
- The near misses are minimal edits of the gold candidate; a model that
  learns edit distance instead of behavior will do well here and badly on
  real code. Multi-defect and refactored near misses are the roadmap outcome
  `non-minimal-near-misses`.
- The families cover the Python ecosystem: library contracts written as
  modern, typed, pythonic code. Algorithm, data-structure, and
  design-pattern families belong to `models/som-code` and are not here.
- The gold candidates pass the quality gate, but pandas and scikit-learn are
  the only data-science libraries imported for real; the PyTorch,
  Playwright, and Selenium families (49, 50, 68 to 70) take duck-typed
  objects the fixture supplies, so they run without a GPU or a browser.

## Provenance

Every family was authored by hand for this corpus: no family was mined from
a GitHub issue, and no candidate is copied from a third-party repository. The library APIs the candidates call are real; the
scenarios are synthetic.
