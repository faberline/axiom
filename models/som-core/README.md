# SOM core

## Brief

`models/som-core` is the engine of the Snippet-Oriented Model (SOM): a
decompression code generator that turns one developer intent into a small
program by planning (L1), laying out the component topology (L2), choosing
the snippet operations that realise it (L3), and assembling the result from a
frozen snippet instruction set (L4). The generator never invents library code;
it selects, parameterises, and places snippets that a corpus project has
already verified.

Every layer generates its record; none is handed candidates to choose
from. Today the engine ships the record schema those layers exchange, a
Python reference assembler that executes an L3 operation list, and the
loader that reads the executable oracle corpus
[`models/som-code-python`](../som-code-python/README.md) owns. No layer
model is trained yet. The [STATUS](STATUS.md) matrix says exactly which
surface is which.

## Direction

Settled 2026-09-25. Where older wording below differs, this section wins.

- **Input is a SOM prompt, not a conversation.** A flagship model (Claude,
  Codex) or a person writes a terse, fixed-order tagged prompt — `intent`,
  `exports`, `patch`, `kwargs`, `raises`, `attrs`, `keys`, `values`,
  `avoid`, `contract:`, `fix:` — the way an image-generation model takes a
  structured prompt. The flagship spends its tokens on the prompt and never
  writes code.
- **The model is a local code generator, not an LLM replacement.** The SOM
  model is fast and lightweight. It runs on-device on macOS (Apple Silicon)
  and translates the prompt into DSL precisely. Reasoning stays with the
  flagship.
- **The model is ours, trained from scratch on our data.** It is never
  fine-tuned from someone else's base model. Claude manufactures the
  training material and the gates verify it, so the model's behaviour comes
  only from the corpus we author. That keeps it controllable, and it is
  retrained as the corpus grows. A model we do not train never produces
  program text, because its output cannot be steered.
- **The DSL is assembled, not free text.** The DSL names ISA snippets and
  their parameters, and `som assemble` renders the program from them, so
  every assembled line comes from an approved snippet.
- **Approved snippets are the optimal form.** Each snippet and each gold
  program passes its family fixture, ruff (including the efficiency rules
  `PERF`, `C4`, `FURB`), mypy strict, and pylint, and is the one canonical
  form of its behaviour. Snippets and their combinations are generated and
  added continuously. A displaced form is kept as a near miss whose
  `why_wrong` names the rule that caught it: negative training data, never
  an inference option.
- **The holdout families are never trained on.**

Current state: the `som_generate` path in use today emits terse Python that
`compile_dsl` checks and expands, from a LoRA on Qwen3-Coder-30B-A3B. Both
are stepping stones. The target is a small from-scratch model emitting
ISA-referencing DSL that `som assemble` assembles.

## Primary workflow

1. Create the project environment with `uv sync` inside `models/som-core`
   and confirm the MLX device with `models/som-core/.venv/bin/som verify`.
2. Assemble a program from an L3 operation list with
   `models/som-core/.venv/bin/som assemble <ops.json|family.json> --out <dir>`;
   a `family.json` is read through its `decompiled.ops`, and snippets come
   from `--isa` (default `models/som-code-python/data/snippets`).
3. Training the layer models is not available: the earlier `train` and
   `serve` commands trained a candidate selector, which SOM is not, and were
   removed; see ROADMAP `layer-sft-on-a-real-backbone`.

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

The records each layer emits are specified in
[docs/reference/layer-records.md](docs/reference/layer-records.md). A layer
model is judged by whether the program its records assemble into passes the
family's oracle fixture; near misses are negative examples for training,
never options at inference.

## Contract discovery

- The `som` entry point is declared in [pyproject.toml](pyproject.toml); the
  Typer app in `src/som_core/cli.py` is the whole CLI surface.
- Training data is read through `src/som_core/dataset.py`, which expects the
  `data/<layer>/families/<family>/` layout that
  [`models/som-code-python`](../som-code-python/README.md) documents and
  passes that project's curation fields (`rationale`, `oracle`, `caption`,
  `plan`, `decompiled`, `why_wrong`, `caught_by`) through to the row; the
  module docstring states the loader's rules.
- `src/som_core/records.py` validates the L1, L2, and L3 records and
  `src/som_core/assemble.py` executes an operation list.
- The layer contract, including the planner's `SCOPE_TOO_LARGE` refusal and
  the L3 operation vocabulary, is the reference document above; the product
  promises per layer live under [docs/product](docs/product/README.md).

## Capabilities

### Capability index

| Capability | ID | User promise | Sources |
|---|---|---|---|
| Training CLI surface | `training-cli-surface` | The `som` entry point installs from this project and lists exactly the `verify` and `assemble` commands with their documented options. | `models/som-core` |
| Corpus loader | `corpus-loader` | The loader unions every corpus directory into one dataset in the order given, tags each row curated or user and samples it by its layer weight (curated=1, user=2 unless `SOM_LAYER_WEIGHTS` says otherwise), carries each family's `caption`, `plan`, and `decompiled` block and each near miss's `why_wrong` and `caught_by` through unchanged, and refuses an empty corpus, a family id that two corpora both claim without a user-layer `overrides`, and a malformed weight. | `models/som-core` |
| Layer records | `layer-records` | `records.py` accepts an L1 plan, L2 topology, and L3 operation list that follow docs/reference/layer-records.md and refuses a missing field, an unknown operation, a reference to an undeclared file or block, and a topology over five files or twenty blocks. | `models/som-core` |
| Reference assembler | `reference-assembler` | `som assemble` writes the files an L3 operation list describes, rendering each snippet from the ISA, and refuses an unknown snippet or a missing or extra parameter. | `models/som-core` |

### Training CLI surface

- ID: `training-cli-surface`
- Promise: The `som` entry point installs from this project and lists
  exactly the `verify` and `assemble` commands with their documented
  options.
- Sources:
  - `models/som-core` owns the Typer app.
- Gate: `models/som-core/.venv/bin/som --help`

### Corpus loader

- ID: `corpus-loader`
- Promise: The loader unions every corpus directory into one dataset in the
  order given, tags each row curated or user and samples it by its layer
  weight (curated=1, user=2 unless `SOM_LAYER_WEIGHTS` says otherwise),
  carries each family's `caption`, `plan`, and `decompiled` block and each
  near miss's `why_wrong` and `caught_by` through unchanged, and refuses an
  empty corpus, a family id that two corpora both claim without a
  user-layer `overrides`, and a malformed weight.
- Sources:
  - `models/som-core` owns the loader and the pytest cases that hold it to this.
- Gate: `models/som-core/.venv/bin/python -m pytest models/som-core/tests -q`

### Layer records

- ID: `layer-records`
- Promise: `records.py` accepts an L1 plan, L2 topology, and L3 operation
  list that follow docs/reference/layer-records.md and refuses a missing
  field, an unknown operation, a reference to an undeclared file or block,
  and a topology over five files or twenty blocks.
- Sources:
  - `models/som-core` owns the schema, the validator, and its pytest cases.
- Gate: `models/som-core/.venv/bin/python -m pytest models/som-core/tests -q`

### Reference assembler

- ID: `reference-assembler`
- Promise: `som assemble` writes the files an L3 operation list describes,
  rendering each snippet from the ISA, and refuses an unknown snippet or a
  missing or extra parameter.
- Sources:
  - `models/som-core` owns the assembler and its pytest cases; the corpus
    project's round-trip gate runs it over every family.
- Gate: `models/som-core/.venv/bin/python -m pytest models/som-core/tests -q`

## Supporting documents

- [STATUS.md](STATUS.md) is the per-surface support matrix, including the
  untrained layers.
- [ROADMAP.md](ROADMAP.md) orders the outcomes: layer SFT and fixture-pass
  evaluation first, then L1 through L4.
- [CONTRIBUTING.md](CONTRIBUTING.md) is the local workflow and verification.
- [docs/product/README.md](docs/product/README.md) indexes the product
  requirement sections.
- [docs/reference/layer-sft-research.md](docs/reference/layer-sft-research.md)
  maps published results on code-in-JSON, constrained decoding, and
  block-wise generation onto the layer SFT failures and orders the next
  experiments.
