# SOM code corpus for Python

## Brief

`models/som-code-python` is the Python corpus project of the Snippet-Oriented
Model. It owns the material the engine in
[`models/som-core`](../som-core/README.md) learns from: executable oracle
families, each a gold program and five near misses judged by a fixture that
runs them, and the snippet instruction set (ISA) the generator assembles
programs from. The `som-code-*` projects exist to teach the model algorithms,
data structures, and design patterns as they appear in working code, one
verified family at a time; the families shipped today cover library-usage
pitfalls, and the algorithm, data-structure, and design-pattern families are
the next outcome on the [roadmap](ROADMAP.md).

The corpus is a data project. Its promises are about what the data proves,
not about a model: every gold candidate passes its fixture, every near miss
fails it on an assertion rather than a crash, and every snippet assembles into
code that compiles.

## Primary workflow

1. Run the oracle harness over every family with
   `models/som-code-python/.venv/bin/python models/som-code-python/data/python-v2/fixtures/verify_harness.py`;
   pass family prefixes such as `08 09 10` to scope it.
2. Run the snippet ISA verifier with
   `models/som-code-python/.venv/bin/python models/som-code-python/scripts/verify_snippets.py`
   after touching anything under `data/snippets/v1/`.
3. Point the engine at the corpus with
   `models/som-core/.venv/bin/som train --data-dir models/som-code-python/data/python-v2`.

## Executable oracle corpus

`data/python-v2/materials/<nn>-<family>/` holds one `family.json` describing
the scenario, a `gold` candidate, and `miss_1` through `miss_5`. The matching
`data/python-v2/fixtures/test_<nn>_<family>.py` is the oracle: it imports the
candidate and asserts behaviour. The harness verifies the Executable Oracle
principle over the whole set and exits non-zero when a gold fails or a near
miss passes or crashes. A family without a fixture is reported and skipped;
the [STATUS](STATUS.md) matrix names the ones in that state.

## Snippet ISA v1

`data/snippets/v1/<library>/<id>.json` is one instruction of the ISA: an `id`,
a `description`, the `imports` it needs, and a Mustache `template` with named
parameters. The verifier checks every required snippet exists, that
parameters are consistent, assembles all of them in dependency order into one
module, and compiles it. Version 1 covers FastAPI, Pydantic, and SQLAlchemy.

## Contract discovery

- The family layout and the harness contract are the docstring of
  `data/python-v2/fixtures/verify_harness.py`; the snippet schema and the
  required set are the constants at the top of `scripts/verify_snippets.py`.
- The engine reads the corpus through `models/som-core`'s dataset loader, so
  a layout change here is a change to that project as well.
- The product promises per area live under
  [docs/product](docs/product/README.md).

## Capabilities

### Capability index

| Capability | ID | User promise | Sources |
|---|---|---|---|
| Executable oracle corpus | `oracle-corpus` | Every family with a fixture has a gold candidate that exits 0 and near misses that fail on an assertion, and the harness exits non-zero on any other outcome. | `models/som-code-python` |
| Snippet ISA v1 | `snippet-isa` | Every required snippet exists with consistent parameters, and the full set assembles in dependency order into a module that compiles. | `models/som-code-python` |

### Executable oracle corpus

- ID: `oracle-corpus`
- Promise: Every family with a fixture has a gold candidate that exits 0 and
  near misses that fail on an assertion, and the harness exits non-zero on
  any other outcome.
- Sources:
  - `models/som-code-python` owns the families, the fixtures, and the harness.
- Gate: `models/som-code-python/.venv/bin/python models/som-code-python/data/python-v2/fixtures/verify_harness.py`

### Snippet ISA v1

- ID: `snippet-isa`
- Promise: Every required snippet exists with consistent parameters, and the
  full set assembles in dependency order into a module that compiles.
- Sources:
  - `models/som-code-python` owns the snippet definitions and the verifier.
- Gate: `models/som-code-python/.venv/bin/python models/som-code-python/scripts/verify_snippets.py`

## Supporting documents

- [STATUS.md](STATUS.md) is the per-surface support matrix, including the
  families without fixtures and the failing legacy tests.
- [ROADMAP.md](ROADMAP.md) orders the outcomes: complete the fixtures, add
  the algorithm and pattern families, emit the decompiler records, then
  packaging.
- [CONTRIBUTING.md](CONTRIBUTING.md) is the local workflow and verification.
- [docs/product/README.md](docs/product/README.md) indexes the product
  requirement sections.
