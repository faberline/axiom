"""The layer-SFT pair builder and lock check refuse what som_core.train's docstring forbids."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from som_core import train
from som_core.dataset import load_corpora, split_dataset
from som_core.wire import dump_ops, load_block, load_ops

ROW = {
    "metadata": {
        "caption": "Create an item and return 201.",
        "plan": {"intent": "Create an item", "target": "Item", "constraints": ["Return 201"]},
        "decompiled": {
            "topology": {"files": [{"path": "main.py", "blocks": [{"name": "app", "kind": "constant", "depends_on": []}]}]},
            "ops": [{"op": "CREATE_FILE", "path": "main.py"}],
        },
    }
}


PLAN = '{"intent":"Create an item","target":"Item","constraints":["Return 201"]}'
TOPOLOGY = '{"files":[{"path":"main.py","blocks":[{"name":"app","kind":"constant","depends_on":[]}]}]}'


def test_every_layer_sees_the_caption_and_every_upstream_record() -> None:
    pairs = train.build_pairs(ROW, l3_mode="whole")
    assert [p[0] for p in pairs] == ["l1", "l2", "l3"]
    assert pairs[0][1] == "CAPTION:\nCreate an item and return 201."
    assert pairs[0][2] == PLAN
    assert pairs[1][1] == f"CAPTION:\nCreate an item and return 201.\n\nL1:\n{PLAN}"
    assert pairs[1][2] == TOPOLOGY
    assert pairs[2][1] == f"CAPTION:\nCreate an item and return 201.\n\nL1:\n{PLAN}\n\nL2:\n{TOPOLOGY}"
    assert pairs[2][2] == '{"op":"CREATE_FILE","path":"main.py"}'


def test_l3_completion_fences_block_source_and_json_format_keeps_the_array() -> None:
    ops = [{"op": "CREATE_FILE", "path": "m.py"}, {"op": "INSERT_BLOCK", "path": "m.py", "block": "f", "source": 'def f():\n    return "}"'}]
    row = {"metadata": {**ROW["metadata"], "decompiled": {**ROW["metadata"]["decompiled"], "ops": ops}}}
    assert train.build_pairs(row, l3_mode="whole")[2][2] == (
        '{"op":"CREATE_FILE","path":"m.py"}\n{"op":"INSERT_BLOCK","path":"m.py","block":"f"}\n'
        '```python\ndef f():\n    return "}"\n```'
    )
    assert json.loads(train.build_pairs(row, l3_format="json", l3_mode="whole")[2][2]) == ops
    with pytest.raises(train.TrainError):
        train.build_pairs(row, l3_format="yaml", l3_mode="whole")


def test_the_l3_system_line_follows_the_format_and_the_json_line_is_unchanged() -> None:
    assert train.SYSTEM["l3"] == "SOM L3 Optimizer: given a topology, emit the operation list as JSON."
    assert train.messages("l3", "p", "c", l3_mode="whole")[0]["content"] == train.SYSTEM_L3_FENCED
    assert train.messages("l3", "p", "c", "json", "whole")[0]["content"] == train.SYSTEM["l3"]
    assert train.messages("l3", "p", "c")[0]["content"] == train.SYSTEM_L3_BLOCK
    assert train.messages("l3i", "p", "c")[0]["content"] == train.SYSTEM_L3I
    assert train.messages("l2", "p", "c")[0]["content"] == train.SYSTEM["l2"]


def test_previous_conditioning_rebuilds_the_prompts_older_adapters_saw() -> None:
    pairs = train.build_pairs(ROW, "previous", l3_mode="whole")
    assert [p[1] for p in pairs] == ["Create an item and return 201.", PLAN, TOPOLOGY]


@pytest.mark.parametrize(
    ("drop", "layers"),
    [("caption", []), ("plan", []), ("decompiled", ["l1"])],
)
def test_a_missing_record_drops_exactly_the_pairs_that_need_it(drop: str, layers: list[str]) -> None:
    row = {"metadata": {k: v for k, v in ROW["metadata"].items() if k != drop}}
    assert [p[0] for p in train.build_pairs(row, l3_mode="whole")] == layers


def test_every_committed_family_yields_three_pairs_and_no_family_straddles_the_split() -> None:
    rows = load_corpora()
    assert all(len(train.build_pairs(r, l3_mode="whole")) == 3 for r in rows)
    tr, va = split_dataset(rows, val_ratio=0.2, seed=42)
    assert va and not {r["id"] for r in tr} & {r["id"] for r in va}


def test_the_holdout_is_the_frozen_005b_val_list_and_never_trains() -> None:
    rows = load_corpora()
    tr, va = train.split_holdout(rows)
    assert {r["id"] for r in va} == train.HOLDOUT and len(train.HOLDOUT) == 20
    assert not {r["id"] for r in tr} & train.HOLDOUT
    assert len(tr) + len(va) == len(rows)
    summary = train.ROOT / "runs" / "layer-sft-005b" / "training_summary.json"
    if summary.exists():
        assert set(json.loads(summary.read_text())["families"]["val"]) == train.HOLDOUT


def test_a_train_fraction_is_nested_deterministic_and_never_reaches_validation() -> None:
    tr, va = split_dataset(train.split_holdout(load_corpora())[0], val_ratio=0.1, seed=42)
    quarter, half = train.subset_families(tr, 0.25), train.subset_families(tr, 0.5)
    ids = lambda rows: {r["id"] for r in rows}  # noqa: E731
    assert len(half) == round(len(tr) / 2) and len(quarter) == round(len(tr) / 4)
    assert ids(quarter) < ids(half) < ids(tr)
    assert ids(train.subset_families(list(reversed(tr)), 0.5)) == ids(half)
    assert not ids(half) & ids(va)
    assert train.subset_families(tr, 1.0) and ids(train.subset_families(tr, 1.0)) == ids(tr)
    for bad in (0.0, 1.5):
        with pytest.raises(train.TrainError, match="not in"):
            train.subset_families(tr, bad)


def test_a_holdout_family_missing_from_the_corpus_is_refused() -> None:
    rows = [r for r in load_corpora() if r["id"] != "som:python:01-fastapi-query-pagination"]
    with pytest.raises(train.TrainError, match="01-fastapi-query-pagination"):
        train.split_holdout(rows)


def test_the_prompt_mask_ends_where_the_completion_begins() -> None:
    class Tok:
        def apply_chat_template(self, msgs, add_generation_prompt=False, return_dict=False):
            text = "".join(f"<{m['role']}>{m['content']}" for m in msgs) + ("<assistant>" if add_generation_prompt else "")
            return [ord(c) for c in text]

    tokens, offset = train._tokenize(Tok(), "l2", "P", "C")
    assert "".join(map(chr, tokens[offset:])) == "C"


def test_a_missing_lock_is_refused(tmp_path: Path) -> None:
    with pytest.raises(train.TrainError, match="run `som lock <model>` first"):
        train.fetch_locked(tmp_path / "sources.lock.json")


def test_a_locked_file_whose_digest_differs_is_refused(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    snapshot = tmp_path / "snap"
    snapshot.mkdir()
    (snapshot / "config.json").write_text("{}")
    lock = tmp_path / "sources.lock.json"
    lock.write_text(json.dumps({"model": "m", "revision": "r", "files": {"config.json": "0" * 64}}))
    import huggingface_hub

    monkeypatch.setattr(huggingface_hub, "snapshot_download", lambda *a, **k: str(snapshot))
    with pytest.raises(train.TrainError, match=r"m@r: config.json sha256 [0-9a-f]{64} != locked 0{64}"):
        train.fetch_locked(lock)


def test_blockwise_pairs_are_one_per_block_plus_one_import_pair_and_rebuild_the_gold_list() -> None:
    rows = load_corpora()
    total = 0
    for row in rows:
        ops = row["metadata"]["decompiled"]["ops"]
        pairs = train.build_pairs(row)
        header, blocks = train.split_ops(ops)
        assert [p[0] for p in pairs] == ["l1", "l2"] + ["l3"] * len(blocks) + ["l3i"], row["id"]
        assert header + blocks == ops, row["id"]
        assert [load_block(p[2], op["path"], op["block"]) for p, op in zip(pairs[2:-1], blocks)] == blocks
        assert load_ops(pairs[-1][2]) == header
        for i, (p, op) in enumerate(zip(pairs[2:-1], blocks)):
            assert p[1].endswith(f"NEXT: {op['path']}:{op['block']}")
            assert (dump_ops(blocks[:i]) or "(none)") in p[1]
        total += len(blocks)
    assert total == 1190


def test_blockwise_needs_the_fenced_cumulative_wire() -> None:
    with pytest.raises(train.TrainError):
        train.build_pairs(ROW, l3_format="json")
    with pytest.raises(train.TrainError):
        train.build_pairs(ROW, "previous")
    with pytest.raises(train.TrainError):
        train.build_pairs(ROW, l3_mode="stream")
