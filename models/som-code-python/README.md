# SOM code corpus for Python

## Brief

`models/som-code-python` is the Python corpus project of the Snippet-Oriented
Model. It owns the material the engine in
[`models/som-core`](../som-core/README.md) learns Python from: executable
oracle families, each a gold program and five near misses judged by a fixture
that runs them, and the snippet instruction set (ISA) the generator assembles
programs from. Its subject is the Python ecosystem: how FastAPI, Pydantic,
SQLAlchemy, asyncio, pandas, scikit-learn, requests, and the rest are used
correctly, written as modern, PEP 8, pythonic code that the community's own
tools accept. Every gold program passes `ruff`, `mypy --strict`, and `pylint`
with the configuration in [pyproject.toml](pyproject.toml). Algorithms, data
structures, and design patterns as such are not this project's subject; they
belong to the language-neutral `som-code` corpus.

The corpus is a data project. Its promises are about what the data proves,
not about a model: every gold candidate passes its fixture and the quality
gate, every near miss fails the fixture on an assertion rather than a crash
while looking exactly as well written as its gold, and every snippet
assembles into code that compiles.

## Primary workflow

1. Create the project virtualenv with
   `uv sync --project models/som-code-python`; it installs every library the
   candidates and fixtures import plus `ruff`, `mypy`, `pylint`, and the stub
   packages.
2. Run the oracle harness over every family with
   `uv run --project models/som-code-python python models/som-code-python/scripts/verify_harness.py`;
   pass family prefixes such as `08 09 10` to scope it.
3. Run the quality gate with
   `uv run --project models/som-code-python python models/som-code-python/scripts/verify_quality.py`
   after changing any candidate; it takes the same family prefixes.
4. Run the curation check with
   `uv run --project models/som-code-python python models/som-code-python/scripts/verify_curation.py`
   after editing any `family.json`; it reads the rationale and caption fields,
   re-measures `decompiled` from the gold source, and never executes a
   candidate.
5. Run the round trip with
   `uv run --project models/som-code-python python models/som-code-python/scripts/verify_roundtrip.py`
   after changing a gold, its `decompiled` block, or the engine's assembler;
   it rebuilds every gold from its operation list with `som assemble`,
   compares the AST, and runs the fixture on the rebuilt file.
6. Run the snippet ISA verifier with
   `uv run --project models/som-code-python python models/som-code-python/scripts/verify_snippets.py`
   after touching anything under `data/snippets/`.
7. The engine's loader reads `data/curated` and, when it holds a family,
   `data/user`; add your own families under `data/user/` as
   [data/user/README.md](data/user/README.md) describes.

## Executable oracle corpus

`data/curated/families/<nn>-<family>/` holds one `family.json` describing
the scenario, a `gold` candidate, and `miss_1` through `miss_5`. The matching
`data/curated/fixtures/test_<nn>_<family>.py` is the oracle: it imports the
candidate and asserts behaviour. The harness verifies the Executable Oracle
principle over the whole set and exits non-zero when a gold fails or a near
miss passes or crashes. A family without a fixture is reported and skipped;
the [STATUS](STATUS.md) matrix names the ones in that state.

Each `family.json` also carries the reason the family is in the corpus: a
`rationale` naming what the gold candidate gets right and why an executable
oracle is needed, the `oracle` fixture the harness must run, and for every
near miss a `why_wrong` sentence plus `caught_by`, the fixture tests that
fail on it, measured by running it. The harness re-measures `caught_by` on
every run and fails on drift; the curation check reads the fields without
executing anything. A `caption` describes the gold program at length, the
way an image model's training caption describes a picture, and is the
planner's prompt; `decompiled` is what the gold source says about itself
(components, third-party imports, raised exceptions, status codes), measured
from its AST, and the check refuses a caption that disagrees with it.
[data/README.md](data/README.md)
explains the shape and the reasoning behind it for anyone who clones the
corpus to train on it.

## Code quality

`scripts/verify_quality.py` holds every candidate to the tools the Python
community runs, configured only by the `[tool.ruff]`, `[tool.mypy]`, and
`[tool.pylint]` tables in `pyproject.toml`: ruff with `E,W,F,I,UP,B,SIM,N,PL,RUF`
at line length 88 for Python 3.12, `ruff format`, mypy in strict mode, and
pylint. A gold candidate must pass all four with zero messages. A near miss
must pass `ruff format` and the style rules `E,W,I,UP`, so formatting,
import order, and modern syntax never tell it apart from its gold, while the
behavioural rules are left out because a near miss's defect may be exactly
what they flag. Every exception in the tool tables carries a comment naming
why the community default does not fit a corpus candidate.

## Snippet ISA

`data/snippets/<library>/<id>.json` is one instruction of the ISA: an `id`,
a `description`, the `imports` it needs, and a `template` of literal text
with `{{name}}` placeholders and one-level `{{#name}}...{{/name}}` list
sections (`models/som-core/docs/reference/layer-records.md` § Snippet
templates). The decompiler turns a gold block into `INSERT_SNIPPET` only when
the extracted parameters render back to the block byte for byte. A snippet
earns its place by use: the verifier counts, from each family's
`decompiled.ops`, how many families use it and refuses one used by fewer
than two, a template line that is only a placeholder, and an operation that
names an id the ISA lacks.

## Contract discovery

- The family layout and the harness contract are the docstring of
  `scripts/verify_harness.py`; the snippet schema and the
  ISA rules are the docstring of `scripts/verify_snippets.py`.
- The engine reads the corpus through `models/som-core`'s dataset loader, so
  a layout change here is a change to that project as well.
- The product promises per area live under
  [docs/product](docs/product/README.md).

## Capabilities

### Capability index

| Capability | ID | User promise | Sources |
|---|---|---|---|
| Executable oracle corpus | `oracle-corpus` | Every family with a fixture has a gold candidate that exits 0 and near misses that fail on exactly the fixture tests their `caught_by` declares, and the harness exits non-zero on any other outcome, including a declared oracle that is not the fixture it ran. | `models/som-code-python` |
| Curated rationale | `curated-rationale` | Every family states what it teaches and why its oracle is executable, every near miss states its observable consequence and the fixture tests that catch it, every gold program has a long caption grounded in its source, and the check exits non-zero on any missing, trivial, label-leaking, or ungrounded field. | `models/som-code-python` |
| Code quality | `code-quality` | Every gold candidate passes ruff, ruff format, mypy in strict mode, and pylint with zero messages, every near miss passes ruff format and the style rules, and the gate exits non-zero naming each candidate that does not. | `models/som-code-python` |
| Decompiled layer records | `decompiled-layer-records` | Every family carries an authored L1 plan and the L2 topology, L3 operation list, and snippet coverage measured from its gold, and `som assemble` rebuilds every gold within the scope limit, and the multi-file TODO fixture, from those records to the same AST, with the family's fixture passing on the rebuilt file. | `models/som-code-python`, `models/som-core` |
| Snippet ISA | `snippet-isa` | Every snippet is used by at least two families' operation lists, has no line that is only a placeholder, and renders its first use to Python, and the verifier reports how many corpus blocks the ISA expresses. | `models/som-code-python`, `models/som-core` |

### Executable oracle corpus

- ID: `oracle-corpus`
- Promise: Every family with a fixture has a gold candidate that exits 0 and
  near misses that fail on exactly the fixture tests their `caught_by`
  declares, and the harness exits non-zero on any other outcome, including a
  declared oracle that is not the fixture it ran.
- Sources:
  - `models/som-code-python` owns the families, the fixtures, and the harness.
- Gate: `uv run --project models/som-code-python python models/som-code-python/scripts/verify_harness.py`

### Curated rationale

- ID: `curated-rationale`
- Promise: Every family states what it teaches and why its oracle is
  executable, every near miss states its observable consequence and the
  fixture tests that catch it, every gold program has a long caption that
  names what its source raises and imports, and the check exits non-zero on
  any missing, trivial, label-leaking, or ungrounded field.
- Sources:
  - `models/som-code-python` owns the rationale and caption fields, the decompiler, the corpus README, and the curation check.
- Gate: `uv run --project models/som-code-python python models/som-code-python/scripts/verify_curation.py`

### Code quality

- ID: `code-quality`
- Promise: Every gold candidate passes ruff, ruff format, mypy in strict
  mode, and pylint with zero messages, every near miss passes ruff format
  and the style rules, and the gate exits non-zero naming each candidate
  that does not.
- Sources:
  - `models/som-code-python` owns the tool configuration in `pyproject.toml` and the quality gate.
- Gate: `uv run --project models/som-code-python python models/som-code-python/scripts/verify_quality.py`

### Decompiled layer records

- ID: `decompiled-layer-records`
- Promise: Every family carries an authored L1 plan and the L2 topology, L3
  operation list, and snippet coverage measured from its gold, and `som
  assemble` rebuilds every gold within the scope limit, and the
  multi-file TODO fixture, from those records to the same AST, with the
  family's fixture passing on the rebuilt file.
- Sources:
  - `models/som-code-python` owns the plans, the decompiler, and the round-trip gate.
  - `models/som-core` owns the record schema and the assembler.
- Gate: `uv run --project models/som-code-python python models/som-code-python/scripts/verify_roundtrip.py`

### Snippet ISA

- ID: `snippet-isa`
- Promise: Every snippet is used by at least two families' operation lists,
  has no line that is only a placeholder, and renders its first use to
  Python, and the verifier reports how many corpus blocks the ISA expresses.
- Sources:
  - `models/som-code-python` owns the snippet definitions, the decompiler that matches them, and the verifier.
  - `models/som-core` owns the template syntax and its renderer.
- Gate: `uv run --project models/som-code-python python models/som-code-python/scripts/verify_snippets.py`

## Supporting documents

- [data/README.md](data/README.md) is the corpus guide
  for a trainer who clones it: why the oracles are executable, what each
  failure mode teaches, the record schema, and provenance.
- [STATUS.md](STATUS.md) is the per-surface support matrix, including the
  families without fixtures.
- [ROADMAP.md](ROADMAP.md) orders the outcomes: complete the fixtures, then
  harden the near misses and cover the ISA's long tail.
- [CONTRIBUTING.md](CONTRIBUTING.md) is the local workflow and verification.
- [docs/product/README.md](docs/product/README.md) indexes the product
  requirement sections.
