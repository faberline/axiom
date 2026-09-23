# Discriminator

What a model trainer gets from the engine today, and what has to change before
the discriminator's score means anything: the training CLI, the corpus
records it should be learning from, and the backbone it should be training.
This area spans the README capabilities `training-cli-surface` and
`corpus-loader`.

## Candidate discriminator training

- Problem: none open as shipped; the limits below belong to the two outcomes
  in this area.
- Who: model trainers running the loop on Apple silicon.
- Promise: `som train --data-dir <corpus>` loads every oracle family under
  `families/`, and with no `--data-dir` loads som-code-python's `data/curated`
  plus `data/user` when it holds a family, trains both layers together
  with user families weighted above curated ones (curated=1, user=2 by
  default, `--layer-weight` to change), runs an MLX LoRA loop for the requested epochs, and writes a
  `training_summary.json` with per-layer validation plus one checkpoint per update to the run
  directory. `som verify` prints the MLX version and device before any
  training starts.
- Limits today: every run trains the randomly initialised skeleton backbone,
  because the repository root in `src/som_core/paths.py` resolves one
  directory short and no base model is ever found; the gold candidate sits in
  a fixed position in every sample, so validation accuracy reads 100 percent
  before the first update; `som serve` preloads the dataset and exits.
- Non-goals: publishing a checkpoint (ROADMAP `published-model-artifact`);
  running a candidate at inference.
- Neighbours: none; first section of the area.
- Status rows: `training-cli-surface`, `discriminator-training-cli`.

## Corpus loader

- Problem: none open as shipped; the limit below belongs to the backbone
  outcome in this area.
- Who: model trainers who train on `som-code`, `som-code-python`, and
  `som-code-rust` in one run; critic authors who need the corpus's
  `why_wrong` and `caught_by` on the row, not re-derived by the engine.
- Promise: `som train` accepts `--data-dir` once per corpus and unions them
  in the order given; every near miss's `why_wrong` and `caught_by` reach its
  training row exactly as the corpus wrote them, and a gold candidate carries
  neither; each family's `caption` and `decompiled` block reach the row's
  `metadata` unchanged, ready for the planner and topology layers; an empty corpus and a family id that two corpora both claim are
  refused with the directories named.
- Limits today: the training loop reads only the candidate texts and the
  gold id, so the curation fields are loaded and not yet trained on.
- Non-goals: rewriting or validating a corpus's rationale (the corpus
  project's curation check owns that); loading a layer record.
- Neighbours: [Candidate discriminator training](#candidate-discriminator-training)
  above, which consumes the rows; [Layer records from the
  decompiler](#layer-records-from-the-decompiler) below, which widens what a
  row carries.
- Status rows: `corpus-loader`.

## Layer records from the decompiler

- Problem: The engine only sees candidate programs and a gold label. Nothing
  it loads says which files, blocks, or snippet operations produced the gold,
  so there is no row a planner, topology, or optimizer model could train on.
- Who: model trainers; corpus authors who need load-time refusal of a bad
  record.
- Promise: The loader reads the L1, L2, and L3 records the corpus project's
  decompiler emits per family, validates all three layers against one schema,
  and refuses a record missing any layer. A training run reports how many
  rows of each layer it loaded.
- Non-goals: the decompiler itself and its lossless reassembly proof, which
  stay in `models/som-code-python`.
- Open: whether the three layers are one JSON document per family or one
  JSONL line per layer; the corpus project's decompiler outcome settles it.
- Neighbours: [Candidate discriminator training](#candidate-discriminator-training)
  above, which this outcome gives real rows to; the corpus project's
  decompiler outcome, which produces the records.
- Outcome: `decompiler-dsl-corpus`. Tracking: Not assigned.

## Discriminator on a real backbone

- Problem: A 100 percent validation accuracy before training is the dataset
  leaking the answer through candidate position, and a skeleton backbone
  cannot learn anything about code. The score the optimizer would rank on is
  currently meaningless.
- Who: model trainers.
- Promise: `som train` loads the base model named in a committed
  `sources.lock.json`, shuffles candidate positions per sample, defaults
  `--skeleton` to off, and validates against a shuffled-position control so an
  initial accuracy near chance is observable in the summary.
- Non-goals: a new model architecture; a published artifact; a second
  language.
- Open: which base model the lock file pins first; the current fallback name
  in `src/som_core/paths.py` is one candidate, not a decision.
- Neighbours: [Layer records from the decompiler](#layer-records-from-the-decompiler)
  above, whose rows this backbone will eventually score.
- Outcome: `discriminator-on-a-real-backbone`. Tracking: Not assigned.

## Non-goals in this area

- No checkpoint under `runs/` is a release; see ROADMAP
  `published-model-artifact`.
- No candidate program runs during scoring; see ROADMAP
  `run-user-code-at-inference`.
