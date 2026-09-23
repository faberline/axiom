# SOM code corpus for Python status

## Scope

This document describes the support contract in the current repository source.
It does not claim that every listed gate ran in this working session.

The rows are the corpus surfaces: the oracle families and their harness, the
curated rationale and its check, the code-quality gate, the snippet ISA and
its verifier, the project's own test suite, and its packaging. Use the
[README](README.md) for the workflow and capabilities. Use the
[roadmap](ROADMAP.md) for future outcomes and explicit non-goals.

## State definitions

| State | Meaning |
|---|---|
| Supported | The current source has a public contract, an implementation, and a named executable gate for the stated scope. |
| Limited | The current source supports the stated scope, but the Limits cell names a material boundary. |
| Not supported | The behavior is not part of the current product contract. The Evidence cell points to a future outcome or a non-goal. |

## Support matrix

| Surface | ID | State | Supported scope | Limits | Evidence |
|---|---|---|---|---|---|
| Executable oracle corpus | `oracle-corpus` | Limited | The harness discovers every family under `data/curated/families`, runs each candidate against its fixture, and exits 0 only when every gold exits 0, every near miss fails on exactly the fixture tests its `caught_by` declares, and every declared `oracle` is the fixture the harness ran; the current run evaluates all 101 families with every gold and every near miss passing that contract. | Families `08-asyncio-concurrency-limiter`, `09-pydantic-field-cross-validation`, and `10-security-timing-constant-auth` have no fixture under `data/curated/fixtures`; the harness prints that no test file matched, skips them, and still exits 0, so their candidates are unproven. See [Oracle fixtures complete](ROADMAP.md#oracle-fixtures-complete). | `uv run --project models/som-code-python python models/som-code-python/scripts/verify_harness.py` |
| Curated rationale | `curated-rationale` | Limited | Every `family.json` carries `rationale.teaches`, `rationale.why`, and `oracle`, every near miss carries `why_wrong` and `caught_by`, and every family carries a `caption` of 80 to 400 words and a `decompiled` block measured from the gold AST; the check refuses a caption that omits a raised exception, declared status code, or imported library, a `decompiled` block that drifted from the source, a missing or trivial field, a `why_wrong` that repeats its failure mode, a `caught_by` naming a test the fixture does not define, a label word in candidate source, and a comment or docstring that differs across the six candidates; the current run reports 0 defects over 101 families. | Families `08-asyncio-concurrency-limiter`, `09-pydantic-field-cross-validation`, and `10-security-timing-constant-auth` have no fixture, so their `oracle` and every `caught_by` under them is `null` and their `why_wrong` sentences are unmeasured prose. See [Oracle fixtures complete](ROADMAP.md#oracle-fixtures-complete). | `uv run --project models/som-code-python python models/som-code-python/scripts/verify_curation.py` |
| Code quality | `code-quality` | Supported | Every gold candidate passes `ruff check` with `E,W,F,I,UP,B,SIM,N,PL,RUF`, `ruff format --check`, `mypy --strict`, and `pylint` with zero messages, and every near miss passes `ruff format --check` and `ruff check --select E,W,I,UP`, all with the tool tables in `pyproject.toml`; the current run reports 0 defects over 101 gold candidates and 505 near misses. | The configuration ignores `PLR2004`, whose literals are the contract the fixture asserts, allows FastAPI and Typer parameter calls in defaults, accepts the `X` feature-matrix name, disables pylint's `too-few-public-methods`, `duplicate-code`, and `not-callable`, and lets mypy skip scikit-learn, which ships no types; each exception carries its reason in `pyproject.toml`. | `uv run --project models/som-code-python python models/som-code-python/scripts/verify_quality.py` |
| Decompiled layer records | `decompiled-layer-records` | Limited | Every `family.json` carries an authored `plan` and a `decompiled` block with `topology`, `ops`, and `coverage` measured from the gold; `som assemble` rebuilds each gold from its `ops` to the same AST and the family's fixture passes on the rebuilt file, and the multi-file TODO fixture under `tests/fixtures/multi_file_todo/` round-trips across its files; the curation check refuses drift and a plan that omits a raised exception or status code; 100 of 101 rebuilt golds and all 5 multi-file sources are byte-identical. | Snippet coverage is 0 of 400 blocks: every block is an `INSERT_BLOCK` literal, because no v1 template matches a whole gold block. `06-sqlalchemy-atomic-order-rollback` has 21 blocks, over the 20-block limit, so `som assemble` refuses it as `SCOPE_TOO_LARGE` and it is not round-tripped. `08`, `09`, and `10` have no fixture and are checked on AST alone. The scope limit is reported by the round trip, not enforced by curation. See [Snippet ISA v2](ROADMAP.md#snippet-isa-v2). | `uv run --project models/som-code-python python models/som-code-python/scripts/verify_roundtrip.py` |
| Snippet ISA v1 | `snippet-isa` | Supported | The eight snippets across `fastapi`, `pydantic`, and `sqlalchemy` exist with consistent Mustache parameters, assemble in dependency order into one module, and that module parses and byte-compiles; `som assemble` renders them through `INSERT_SNIPPET`. | The library set is fixed at three. | `uv run --project models/som-code-python python models/som-code-python/scripts/verify_snippets.py` |
| Project test suite | `python-test-suite` | Supported | `tests/` holds the snippet adversarial cases, which run under the project virtualenv and pass: 20 passed. | None. | `uv run --project models/som-code-python python -m pytest models/som-code-python/tests -q` |
| uv-runnable packaging | `uv-runnable-packaging` | Supported | `uv sync --project models/som-code-python` resolves from `uv.lock` and installs every third-party library the candidates and fixtures import, plus ruff, mypy, pylint, and their stub packages, into the project `.venv`; every gate here runs through `uv run --project models/som-code-python`. | The project is not a package (`[tool.uv] package = false`) and declares no console script; the engine's `som` command comes from `models/som-core`. | `uv sync --project models/som-code-python` |

## Evidence policy

- A Supported or Limited row names the command that measures it, run from the
  repository root through `uv run --project models/som-code-python`.
- A Limited row's Limits cell states the boundary as an observation and names
  every tolerated failure verbatim, never as a count, and links the ROADMAP
  outcome that removes it.
- A Not supported row links exactly one ROADMAP outcome or non-goal and claims
  no gate.
- The harness exiting 0 is not proof for a family it skipped; the Limits cell
  of the oracle row is where a skipped family is recorded.
