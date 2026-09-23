# Training

What a model trainer gets from the engine: a loader that reads every corpus
into one weighted dataset, the layer records each generation layer learns
from, and the training and evaluation loop those records are for. SOM is a
generator. Every layer model is trained to emit the next layer's record
token by token; no layer is handed a list of candidates to choose from, at
training time or at inference. This area spans the README capabilities
`training-cli-surface`, `corpus-loader`, and `layer-records`.

## Corpus loader

- Problem: none open as shipped; the limit below belongs to the training
  outcome in this area.
- Who: model trainers who train on `som-code`, `som-code-python`, and
  `som-code-rust` together; corpus authors whose curated and user layers
  must both reach the dataset.
- Promise: The loader unions every corpus directory in the order given, and
  with none given loads som-code-python's `data/curated` plus `data/user`
  when it holds a family. Every row carries its layer (`curated`, or `user`
  for a corpus directory named `user`), and a user family's `overrides`
  replaces the named curated row. Sampling repeats each row by its layer
  weight (curated=1, user=2 by default, `SOM_LAYER_WEIGHTS` to change) and the
  validation split keeps rows of every layer. Each family's `caption`,
  `plan`, and `decompiled` block, and each near miss's `why_wrong` and
  `caught_by`, reach the row unchanged. An empty corpus, a family id two
  corpora claim without `overrides`, an `overrides` in the curated layer, an
  unloaded target, two overrides of one target, and a malformed weight are
  refused with the directories named.
- Limits today: nothing trains on the rows; see
  [Layer SFT on a real backbone](#layer-sft-on-a-real-backbone).
- Non-goals: rewriting or validating a corpus's rationale, which the corpus
  project's curation check owns.
- Neighbours: none; first section of the area.
- Status rows: `corpus-loader`.

## Layer records

- Problem: none open as shipped.
- Who: model trainers, whose training pairs are one layer's record in and
  the next layer's record out; corpus authors, whose decompiler emits the
  records.
- Promise: The L1 plan, L2 topology, and L3 operation list have one schema,
  written in [docs/reference/layer-records.md](../reference/layer-records.md)
  and enforced by `src/som_core/records.py`. The validator refuses a missing
  field, an unknown operation, an operation that names a file or block the
  topology does not declare, a dependency on an undeclared block, and a
  topology over five files or twenty blocks, which is the planner's
  `SCOPE_TOO_LARGE` refusal.
- Non-goals: the decompiler that emits the records, which stays in
  `models/som-code-python`.
- Neighbours: [Corpus loader](#corpus-loader) above, which carries the
  records on every row; the generation area, whose layers emit them.
- Status rows: `layer-records`.

## Layer SFT on a real backbone

- Problem: No loop trains anything. The previous loop scored candidate
  programs against each other, the JEV selector shape that SOM is not, on a
  randomly initialised skeleton backbone; it has been removed.
- Who: model trainers.
- Promise: `som train` loads the base model a committed `sources.lock.json`
  names, and fine-tunes it with LoRA and next-token loss on three pair sets
  built from the loaded rows: caption to L1 plan, L1 plan to L2 topology,
  and L2 topology to L3 operation list. Rows are sampled by layer weight, and
  the run writes a summary with per-layer, per-pair validation loss.
- Non-goals: a new model architecture; a published artifact; a second
  language.
- Open: one adapter per pair or one adapter with a pair tag in the prompt;
  which base model the lock file pins first.
- Neighbours: [Layer records](#layer-records) above, whose records are the
  pairs; [Fixture pass rate](#fixture-pass-rate) below, which judges the
  result.
- Outcome: `layer-sft-on-a-real-backbone`. Tracking: Not assigned.

## Fixture pass rate

- Problem: Loss says how well a model imitates records, not whether the
  program it plans runs. An accuracy over a candidate list measured the
  wrong task and is gone with the selector.
- Who: model trainers deciding whether a checkpoint is better.
- Promise: `som eval` generates L1, L2, and L3 for every held-out family
  from its caption alone, assembles the L3 operation list, runs the family's
  oracle fixture on the assembled files, and reports the share of families
  whose fixture exits 0, alongside the quality gate's result on the same
  files.
- Non-goals: running assembled code outside a fixture; scoring against near
  misses.
- Open: whether a family whose generated plan is refused as
  `SCOPE_TOO_LARGE` counts as a failure or is reported apart.
- Neighbours: [Layer SFT on a real backbone](#layer-sft-on-a-real-backbone)
  above, whose checkpoints this judges.
- Outcome: `fixture-pass-rate-evaluation`. Tracking: Not assigned.

## Near-miss preference data

- Problem: Five near misses per family are measured defects with a named
  failing test, and the SFT pairs use none of them.
- Who: model trainers.
- Promise: Each near miss is decompiled to its own L3 operation list, and a
  preference pass (DPO) trains on gold versus near-miss operation lists for
  the same L2 topology, so the optimizer learns what not to emit.
- Non-goals: offering near misses as choices at inference; ROADMAP
  `candidate-selection-at-inference`.
- Open: whether the preference pass runs after SFT or interleaved.
- Neighbours: [Layer SFT on a real backbone](#layer-sft-on-a-real-backbone)
  above.
- Outcome: `near-miss-preference-data`. Tracking: Not assigned.

## Non-goals in this area

- No layer picks from candidates a caller supplies; see ROADMAP
  `candidate-selection-at-inference`.
- No checkpoint under `runs/` is a release; see ROADMAP
  `published-model-artifact`.
