# SOM core

## Brief

`models/som-core` is the engine of the Snippet-Oriented Model (SOM): a
decompression code generator that turns one developer intent into a small
program by planning (L1), laying out the component topology (L2), choosing
the snippet operations that realise it (L3), and assembling the result from a
frozen snippet instruction set (L4). The generator never invents library code;
it selects, parameterises, and places snippets that a corpus project has
already verified.

Today the engine ships one piece of that chain: a candidate discriminator
trained with MLX LoRA on the executable oracle corpus that
[`models/som-code-python`](../som-code-python/README.md) owns. The four
generation layers are documented, not implemented. The
[STATUS](STATUS.md) matrix says exactly which surface is which.

## Primary workflow

1. Create the project environment with `uv sync` inside `models/som-core`
   and confirm the MLX device with `models/som-core/.venv/bin/som verify`.
2. Train the discriminator against one or more corpus directories with
   `models/som-core/.venv/bin/som train --data-dir models/som-code-python/data/python-v2`,
   repeating `--data-dir` once per corpus to train on `som-code`,
   `som-code-python`, and `som-code-rust` together (add `--smoke` for a
   bounded run); the run directory records a `training_summary.json` and one
   checkpoint per update step.
3. Serve the checkpoint with `models/som-core/.venv/bin/som serve` once a
   real backbone exists; today the command only preloads the dataset and
   prints the stub banner.

## Layer model

The generator is four layers over one instruction set, described in
[docs/reference/architecture.md](docs/reference/architecture.md):

- L1 Planner reads the intent and refuses a scope wider than a handful of
  files or blocks before any code is planned.
- L2 Component Topology fixes which files and blocks exist and how they
  depend on each other.
- L3 Query Optimizer selects the snippet operations, with parameters, that
  fill each block.
- L4 Assembler executes the operation list against the snippet ISA and
  writes the files.

The discriminator is the model that scores candidate programs so the
optimizer can prefer the gold over a near miss. It is the only trained
component in this repository.

## Contract discovery

- The `som` entry point is declared in [pyproject.toml](pyproject.toml); the
  Typer app in `src/som_core/cli.py` is the whole CLI surface.
- Training data is read through `src/som_core/dataset.py`, which expects the
  `data/python-v2/materials/<family>/` layout that
  [`models/som-code-python`](../som-code-python/README.md) documents and
  passes that project's curation fields (`rationale`, `oracle`, `caption`,
  `decompiled`, `why_wrong`, `caught_by`) through to the training row; the module docstring states the
  loader's rules.
- The layer contract, including the planner's `SCOPE_TOO_LARGE` refusal and
  the L3 operation vocabulary, is the reference document above; the product
  promises per layer live under [docs/product](docs/product/README.md).

## Capabilities

### Capability index

| Capability | ID | User promise | Sources |
|---|---|---|---|
| Training CLI surface | `training-cli-surface` | The `som` entry point installs from this project and lists the `verify`, `train`, and `serve` commands with their documented options. | `models/som-core` |
| Corpus loader | `corpus-loader` | `som train` unions every `--data-dir` corpus into one dataset in the order given, carries each near miss's `why_wrong` and `caught_by` through to its training row unchanged, and refuses an empty corpus or a family id that two corpora both claim. | `models/som-core` |

### Training CLI surface

- ID: `training-cli-surface`
- Promise: The `som` entry point installs from this project and lists the
  `verify`, `train`, and `serve` commands with their documented options.
- Sources:
  - `models/som-core` owns the Typer app and the training loop behind it.
- Gate: `models/som-core/.venv/bin/som --help`

### Corpus loader

- ID: `corpus-loader`
- Promise: `som train` unions every `--data-dir` corpus into one dataset in
  the order given, carries each near miss's `why_wrong` and `caught_by`
  through to its training row unchanged, and refuses an empty corpus or a
  family id that two corpora both claim.
- Sources:
  - `models/som-core` owns the loader and the pytest cases that hold it to this.
- Gate: `models/som-core/.venv/bin/python -m pytest models/som-core/tests -q`

## Supporting documents

- [STATUS.md](STATUS.md) is the per-surface support matrix, including the
  discriminator's training limits and the four unimplemented layers.
- [ROADMAP.md](ROADMAP.md) orders the outcomes: data first, then the
  discriminator backbone, then L1 through L4.
- [CONTRIBUTING.md](CONTRIBUTING.md) is the local workflow and verification.
- [docs/product/README.md](docs/product/README.md) indexes the product
  requirement sections.
