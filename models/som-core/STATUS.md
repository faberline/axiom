# SOM core status

## Scope

This document describes the support contract in the current repository source.
It does not claim that every listed gate ran in this working session.

The rows are the engine surfaces: the training CLI, the corpus loader
behind it, the discriminator it trains, the four generation layers of the layer model, and the base-model
provenance the training loop depends on. Use the [README](README.md) for the
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
| Training CLI surface | `training-cli-surface` | Supported | The `som` entry point lists `verify`, `train`, and `serve`; `verify` prints the MLX version and device; `train` accepts `--domain`, a repeatable `--data-dir`, `--epochs`, `--batch-size`, `--lr`, `--smoke`, `--output-dir`, and `--skeleton`. | `serve` is a stub that preloads the dataset and exits; there is no request loop. | `models/som-core/.venv/bin/som --help` |
| Corpus loader | `corpus-loader` | Supported | Every `--data-dir` corpus is loaded in the order given into one dataset; a near miss's `why_wrong` and `caught_by` reach its training row unchanged and a gold candidate carries neither; each family's `caption` and `decompiled` block reach the row's metadata unchanged; an empty corpus and a family id claimed by two corpora are refused with the directories named; the python-v2 corpus loads as 101 families with measured `caught_by` on every near miss of the 98 families that have an oracle and a `caption` on all 101. | The training loop reads only the candidate texts and the gold id; `why_wrong`, `caught_by`, `caption`, and `decompiled` are loaded, not yet trained on. | `models/som-core/.venv/bin/python -m pytest models/som-core/tests -q` |
| Discriminator training | `discriminator-training-cli` | Limited | `som train` loads every family under `data/python-v2/materials`, runs an MLX LoRA loop, and writes `training_summary.json` plus one checkpoint per update to the run directory; a `--smoke` run over the 101 Python families completes with exit 0. | Every run trains the randomly initialised skeleton backbone because `src/som_core/paths.py` resolves the repository root one directory short and never finds a base model; the dataset keeps the gold candidate in a fixed position, so validation accuracy reads 100 percent before the first update. See [Discriminator on a real backbone](ROADMAP.md#discriminator-on-a-real-backbone). | `models/som-core/.venv/bin/som train --smoke --data-dir models/som-code-python/data/python-v2 --output-dir models/som-core/runs/smoke` |
| Base model provenance | `base-model-sources-lock` | Not supported | None. | No `sources.lock.json` names the base model, its revision, or its digest, and no command downloads one. | [Discriminator on a real backbone](ROADMAP.md#discriminator-on-a-real-backbone) |
| L1 Planner | `l1-planner` | Not supported | None. | No module reads an intent, emits the L1 plan, or refuses an oversized scope. | [L1 Planner](ROADMAP.md#l1-planner) |
| L2 Component Topology | `l2-topology` | Not supported | None. | No module emits the file and block layout. | [L2 Component Topology](ROADMAP.md#l2-component-topology) |
| L3 Query Optimizer | `l3-optimizer` | Not supported | None. | No module selects snippet operations for a topology, and the discriminator is not wired to score them. | [L3 Query Optimizer](ROADMAP.md#l3-query-optimizer) |
| L4 Assembler | `l4-assembler` | Not supported | None. | No assembler executes an operation list against the snippet ISA; the ISA verifier in the corpus project is the only consumer of the snippets. | [L4 Rust Assembler](ROADMAP.md#l4-rust-assembler) |

## Evidence policy

- A Supported or Limited row names the command that measures it, run from the
  repository root after `uv sync` inside `models/som-core`, which installs the
  `dev` dependency group the pytest gate needs.
- A Limited row's Limits cell states the boundary as an observation, never as
  a count, and links the ROADMAP outcome that removes it.
- A Not supported row links exactly one ROADMAP outcome or non-goal and claims
  no gate.
- The smoke training gate proves the loop runs end to end; it does not prove
  the model learned anything, which is the Limits cell's point.
