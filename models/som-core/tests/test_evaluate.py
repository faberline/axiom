"""som eval classifies each held-out family by the first stage its generated chain fails."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from som_core import evaluate, train
from som_core.assemble import DEFAULT_ISA, load_isa
from som_core.dataset import find_default_corpora, load_corpus
from som_core.wire import dump_block, dump_ops

CURATED = find_default_corpora()[0]
ROW = next(r for r in load_corpus(CURATED) if r["family"] == "00-fastapi-item-create-201")
GOLD = {
    "l1": json.dumps(ROW["metadata"]["plan"]),
    "l2": json.dumps(ROW["metadata"]["decompiled"]["topology"]),
    "l3": json.dumps(ROW["metadata"]["decompiled"]["ops"]),
}


def _chain(monkeypatch: pytest.MonkeyPatch, tmp_path: Path, given: dict | None = None, until: str = "l3", **override: str) -> dict:
    outputs = {**GOLD, **override}
    prompts: dict[str, str] = {}

    def fake(_m, _t, layer: str, prompt: str, *_fmt: str) -> str:
        prompts[layer] = prompt
        return outputs[layer]

    monkeypatch.setattr(evaluate, "_generate", fake)
    result = evaluate.run_chain(None, None, ROW["metadata"]["caption"], load_isa(DEFAULT_ISA), tmp_path / "out", given=given, until=until)
    return {**result, "prompts": prompts}


def test_the_gold_records_assemble_and_pass_the_real_fixture(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    assert _chain(monkeypatch, tmp_path)["stage"] == "assembled"
    verdict = evaluate.judge(CURATED, ROW["family"], tmp_path / "out")
    assert verdict["fixture"] == 0 and verdict["quality"] == []


@pytest.mark.parametrize(
    ("override", "stage"),
    [
        ({"l1": "{not json"}, "l1"),
        ({"l1": '{"intent": "x"}'}, "l1"),
        ({"l2": '{"files": []}'}, "l2"),
        ({"l3": "[{"}, "l3"),
        ({"l3": '[{"op": "INSERT_BLOCK", "path": "ghost.py", "block": "x", "source": "x = 1"}]'}, "assemble"),
    ],
    ids=["l1-not-json", "l1-missing-field", "l2-no-file", "l3-not-json", "assemble-undeclared-file"],
)
def test_a_broken_stage_stops_the_chain_by_name(monkeypatch: pytest.MonkeyPatch, tmp_path: Path, override: dict, stage: str) -> None:
    result = _chain(monkeypatch, tmp_path, **override)
    assert result["stage"] == stage
    assert not (tmp_path / "out" / "candidate.py").exists()


def test_fixture_lookup_matches_the_harness() -> None:
    assert evaluate.has_fixture(CURATED, "00-fastapi-item-create-201")
    assert not evaluate.has_fixture(CURATED, "08-asyncio-concurrency-limiter")


def test_the_chain_prompts_carry_the_caption_and_the_generated_upstream_records(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    prompts = _chain(monkeypatch, tmp_path)["prompts"]
    caption = ROW["metadata"]["caption"]
    assert all(p.startswith(f"CAPTION:\n{caption}") for p in prompts.values())
    assert "\n\nL1:\n" in prompts["l3"] and "\n\nL2:\n" in prompts["l3"]


def test_teacher_forced_l3_generates_only_l3_from_the_gold_records(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    meta = ROW["metadata"]
    given = {"l1": meta["plan"], "l2": meta["decompiled"]["topology"]}
    result = _chain(monkeypatch, tmp_path, given=given, l1="{broken", l2="{broken")
    assert result["stage"] == "assembled" and list(result["prompts"]) == ["l3"]


def test_teacher_forced_l2_stops_after_l2(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    ok = _chain(monkeypatch, tmp_path, given={"l1": ROW["metadata"]["plan"]}, until="l2")
    assert ok["stage"] == "ok" and list(ok["prompts"]) == ["l2"]
    assert not (tmp_path / "out" / "candidate.py").exists()
    bad = _chain(monkeypatch, tmp_path, given={"l1": ROW["metadata"]["plan"]}, until="l2", l2='{"files": []}')
    assert bad["stage"] == "l2"


def test_a_fenced_adapter_s_l3_text_is_read_by_the_wire_parser(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    fenced = dump_ops(ROW["metadata"]["decompiled"]["ops"])
    monkeypatch.setattr(evaluate, "_generate", lambda _m, _t, layer, _p, fmt, _mode: fenced if fmt == "fenced" else GOLD[layer])
    given = {"l1": ROW["metadata"]["plan"], "l2": ROW["metadata"]["decompiled"]["topology"]}
    isa = load_isa(DEFAULT_ISA)
    assert evaluate.run_chain(None, None, "c", isa, tmp_path / "a", given=given, l3_format="fenced")["stage"] == "assembled"
    assert evaluate.run_chain(None, None, "c", isa, tmp_path / "b", given=given, l3_format="json")["stage"] == "assembled"
    monkeypatch.setattr(evaluate, "_generate", lambda *_a: fenced)
    assert evaluate.run_chain(None, None, "c", isa, tmp_path / "c", given=given, l3_format="json")["stage"] == "l3"


def _blockwise(monkeypatch: pytest.MonkeyPatch, tmp_path: Path, mutate=None) -> dict:
    header, blocks = train.split_ops(ROW["metadata"]["decompiled"]["ops"])
    calls: list[tuple[str, str]] = []

    def fake(_m, _t, layer: str, prompt: str, fmt: str, mode: str) -> str:
        assert (fmt, mode) == ("fenced", train.L3_MODE)
        calls.append((layer, prompt))
        if layer == "l3i":
            return dump_ops(header)
        op = blocks[sum(1 for c in calls if c[0] == "l3") - 1]
        return mutate(op) if mutate else dump_block(op)

    monkeypatch.setattr(evaluate, "_generate", fake)
    given = {"l1": ROW["metadata"]["plan"], "l2": ROW["metadata"]["decompiled"]["topology"]}
    result = evaluate.run_chain(None, None, ROW["metadata"]["caption"], load_isa(DEFAULT_ISA), tmp_path / "out",
                                given=given, l3_format="fenced", l3_mode=train.L3_MODE)
    return {**result, "calls": calls, "blocks": blocks}


def test_blockwise_l3_calls_once_per_topology_block_then_imports_and_assembles(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    result = _blockwise(monkeypatch, tmp_path)
    blocks = result["blocks"]
    assert result["stage"] == "assembled"
    assert [c[0] for c in result["calls"]] == ["l3"] * len(blocks) + ["l3i"]
    assert result["calls"][0][1].endswith(f"DONE:\n(none)\n\nNEXT: {blocks[0]['path']}:{blocks[0]['block']}")
    assert dump_ops(blocks[:1]) in result["calls"][1][1]
    assert result["calls"][-1][1].endswith(f"BLOCKS:\n{dump_ops(blocks)}")


def test_blockwise_l3_refuses_a_reply_for_another_block(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    result = _blockwise(monkeypatch, tmp_path, lambda op: dump_ops([op]))
    assert result["stage"] == "l3" and "the header carries ['path', 'block']" in result["error"]
    assert len(result["calls"]) == 1


def test_no_repeat_ngram_bans_only_the_token_completing_a_repeated_run() -> None:
    import mlx.core as mx

    from som_core.evaluate import no_repeat_ngram

    ban = no_repeat_ngram(3, prompt_len=2)
    assert ban(mx.array([1, 2, 1, 2, 3, 1, 2]), mx.zeros((1, 5))).tolist() == [[0.0, 0.0, 0.0, float("-inf"), 0.0]]
    # the prompt's own repeats are not the reply's
    assert ban(mx.array([1, 2, 3, 1, 2]), mx.zeros((1, 5))).tolist() == [[0.0] * 5]
