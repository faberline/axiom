# SOM core status

## Scope

This document describes the support contract in the current repository source.
It does not claim that every listed gate ran in this working session.

The rows are the engine surfaces: the CLI, the corpus loader, the layer
records and the reference assembler, layer training and the base-model
provenance it depends on, and the four generation layers. Use the [README](README.md) for the
workflow and capabilities. Use the [roadmap](ROADMAP.md) for future outcomes
and explicit non-goals.

## State definitions

| State | Meaning |
|---|---|
| Supported | The current source has a public contract, an implementation, and a named executable gate for the stated scope. |
| Limited | The current source supports the stated scope, but the Limits cell names a material boundary. |
| Not supported | The behavior is not part of the current product contract. The Evidence cell points to a future outcome or a non-goal. |

## Support matrix

| Surface | ID | State | Supported scope | Limits | Evidence |
|---|---|---|---|---|---|
| Training CLI surface | `training-cli-surface` | Supported | The `som` entry point lists exactly `verify` and `assemble`; `verify` prints the MLX version and device; `assemble` takes an operation list or `family.json`, `--out`, and `--isa`. | None. | `models/som-core/.venv/bin/som --help` |
| Corpus loader | `corpus-loader` | Limited | Every corpus directory is loaded in the order given into one dataset, and with none given som-code-python's `data/curated` is loaded followed by `data/user` when it holds a family; a near miss's `why_wrong` and `caught_by` reach its row unchanged and a gold candidate carries neither; each family's `caption`, `plan`, and `decompiled` block reach the row's metadata unchanged; every row carries its layer (`curated`, or `user` for a corpus directory named `user`) and a user family's `overrides` replaces the named row, while an empty corpus, a family id claimed by two corpora without `overrides`, an `overrides` in the curated layer, an unloaded target, and two overrides of one target are refused with the directories named; sampling oversamples each layer by its weight (default curated=1, user=2, `SOM_LAYER_WEIGHTS` to change, malformed weights refused) and the validation split keeps rows of every layer. | Nothing trains on the rows. See [Layer SFT on a real backbone](ROADMAP.md#layer-sft-on-a-real-backbone). | `models/som-core/.venv/bin/python -m pytest models/som-core/tests -q` |
| Layer records | `layer-records` | Supported | `records.py` validates the L1 plan, L2 topology, and L3 operation list of docs/reference/layer-records.md, refusing a missing field, an unknown operation, a reference to an undeclared file or block, a dependency on an undeclared block, a block inserted twice or never, and a topology over five files or twenty blocks (`SCOPE_TOO_LARGE`). | None. | `models/som-core/.venv/bin/python -m pytest models/som-core/tests -q` |
| Reference assembler | `reference-assembler` | Supported | `som assemble` writes every file of an operation list, rendering `INSERT_SNIPPET` from the ISA, including one-level list sections rendered once per item, and writing `INSERT_BLOCK` verbatim, and refuses an unknown snippet id, a missing or extra parameter in `params` or in any section item, and a nested, unclosed, or mismatched section; the corpus project's round-trip gate reassembles every curated family to its gold's AST. | None. | `models/som-core/.venv/bin/python -m pytest models/som-core/tests -q` |
| Layer training | `layer-training` | Not supported | None. | No command trains a layer model; the candidate-selection loop was removed. | [Layer SFT on a real backbone](ROADMAP.md#layer-sft-on-a-real-backbone) |
| Base model provenance | `base-model-sources-lock` | Not supported | None. | No `sources.lock.json` names the base model, its revision, or its digest, and no command downloads one. | [Layer SFT on a real backbone](ROADMAP.md#layer-sft-on-a-real-backbone) |
| L1 Planner | `l1-planner` | Not supported | None. | No model emits the L1 plan or refuses an oversized scope; the plans exist only as authored training records. | [L1 Planner](ROADMAP.md#l1-planner) |
| L2 Component Topology | `l2-topology` | Not supported | None. | No model emits the file and block layout; the topologies exist only as decompiled training records. | [L2 Component Topology](ROADMAP.md#l2-component-topology) |
| L3 Query Optimizer | `l3-optimizer` | Not supported | None. | No model emits an operation list; the operation lists exist only as decompiled training records. | [L3 Query Optimizer](ROADMAP.md#l3-query-optimizer) |
| L4 Assembler | `l4-assembler` | Not supported | None. | The Python reference assembler exists (`reference-assembler`); the Rust assembler does not. | [L4 Rust Assembler](ROADMAP.md#l4-rust-assembler) |

## Evidence policy

- A Supported or Limited row names the command that measures it, run from the
  repository root after `uv sync` inside `models/som-core`, which installs the
  `dev` dependency group the pytest gate needs.
- A Limited row's Limits cell states the boundary as an observation, never as
  a count, and links the ROADMAP outcome that removes it.
- A Not supported row links exactly one ROADMAP outcome or non-goal and claims
  no gate.
