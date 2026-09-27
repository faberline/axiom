# Your own training families

Put families you want to train on here. Nothing under this directory except
this file is committed; it is yours.

```
user/
  families/<nn>-<name>/family.json   same schema as ../curated/families
  families/<nn>-<name>/gold/...      one file per candidate the family lists
  fixtures/test_<nn>_<name>.py       the oracle the family's `oracle` names
```

`som train` with no `--data-dir` picks this layer up as soon as it holds one
`family.json` and trains it together with the curated layer. Your families
are your team's conventions, so they weigh more by default: `user=2`,
`curated=1`. Change it per run with `som train -w user=3` or for the shell
with `SOM_LAYER_WEIGHTS=user=3,curated=1`; `user=0` keeps your families in
validation only. `training_summary.json` reports validation per layer.

A family id that the curated layer already uses is refused, so pick a
number range of your own (for example `900-` and up). To replace a curated
family with your team's version, say so in your `family.json`:

```
"overrides": "som:python:00-fastapi-item-create-201"
```

The curated row is then dropped and yours trains in its place. The loader
refuses an override whose target is not loaded, two of your families
overriding the same target, and an id collision without `overrides`.

Hold your families to the curated bar before training on them, from the
repository root:

```
uv run --project models/som-code-python python models/som-code-python/scripts/verify_harness.py --corpus models/som-code-python/data/user
uv run --project models/som-code-python python models/som-code-python/scripts/decompile_gold.py --corpus models/som-code-python/data/user --write
uv run --project models/som-code-python python models/som-code-python/scripts/verify_curation.py --corpus models/som-code-python/data/user
uv run --project models/som-code-python python models/som-code-python/scripts/verify_quality.py --corpus models/som-code-python/data/user
```

The loader does not run these gates for you: an unchecked family trains
like a checked one. Read [../README.md](../README.md) for why each field
exists.
