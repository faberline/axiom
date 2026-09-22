import copy

import pytest

from som.developer.coder_protocol import freeze, verify as verify_protocol
from som.developer.coder_records import collect
from som.developer.data import DATA, REVIEW_ID
from som.developer.final_cases import FINAL, cases, rows as final_rows
from som.developer.fixtures import fixtures
from som.paths import read_json, read_rows, write_json


@pytest.mark.skipif(not (FINAL / "manifest.json").exists(), reason="Fresh data not prepared")
def test_fresh_coder_holdout_has_no_development_family_or_instance():
    rows = final_rows()
    assert len(rows) == 240
    assert not {f.name for f in cases()} & {f.name for f in fixtures()}
    development = [r for split in ("train", "validation", "calibration", "test", "challenge")
                   for r in read_rows(DATA / f"{split}.jsonl")]
    assert not {r["id"] for r in rows} & {r["id"] for r in development}
    assert not {r["state"] for r in rows} & {r["state"] for r in development}
    for domain in ("python", "frontend"):
        selected = [r for r in rows if r["task"] == domain]
        assert len(selected) == 120
        assert sum(r["gold_candidate_id"] == REVIEW_ID for r in selected) == 36
    assert all(v == [True, False, False, False] for v in read_json(FINAL / "manifest.json")["oracle_results"].values())


@pytest.mark.skipif(not (FINAL / "manifest.json").exists(), reason="Fresh data not prepared")
def test_coder_acceptance_cannot_change_silently(tmp_path):
    freeze(tmp_path)
    verify_protocol(tmp_path)
    value = read_json(tmp_path / "acceptance-protocol.json")
    value["criteria"]["fresh_family_accuracy"][0] = .1
    write_json(tmp_path / "acceptance-protocol.json", value)
    with pytest.raises(ValueError, match="protocol"):
        freeze(tmp_path)
    with pytest.raises(ValueError, match="protocol"):
        verify_protocol(tmp_path)


def evaluation_rows(n):
    return [{"id": str(i), "task": "python", "family": "test", "state": "state", "question": "choose",
             "candidates": [{"id": "a", "text": "first"}, {"id": "b", "text": "second"}],
             "gold_candidate_id": "a"} for i in range(n)]


def test_evaluation_resumes_without_changing_candidate_mapping(tmp_path):
    rows = evaluation_rows(40)
    calls = []
    def interrupted(row):
        calls.append(row["id"])
        if len(calls) == 36:
            raise RuntimeError("interrupted")
        return {"logits": [2., 0.], "seconds": .01}
    destination = tmp_path / "scores.json"
    with pytest.raises(RuntimeError, match="interrupted"):
        collect(rows, interrupted, destination)
    assert len(read_json(tmp_path / "scores.partial.json")) == 32
    resumed = []
    def scorer(row):
        resumed.append(row["id"])
        return {"logits": [2., 0.], "seconds": .01}
    records = collect(rows, scorer, destination)
    assert resumed == ["32", *map(str, range(32, 40))]
    assert [r["id"] for r in records] == list(map(str, range(40)))
    collect(rows, lambda _: pytest.fail("Completed cache must not score again"), destination)
    changed = copy.deepcopy(rows)
    changed[0]["candidates"].reverse()
    with pytest.raises(ValueError, match="inputs changed"):
        collect(changed, scorer, destination)


def test_evaluation_rejects_wrong_length_and_nonfinite_scores(tmp_path):
    for i, logits in enumerate(([1.], [float("nan"), 0.])):
        with pytest.raises(RuntimeError, match="Invalid evaluation logits"):
            collect(evaluation_rows(1), lambda _: {"logits": logits}, tmp_path / f"{i}.json")


def test_completed_cache_cannot_hide_missing_records(tmp_path):
    destination = tmp_path / "scores.json"
    rows = evaluation_rows(2)
    collect(rows, lambda _: {"logits": [1., 0.], "seconds": .01}, destination)
    write_json(destination, read_json(destination)[:1])
    with pytest.raises(ValueError, match="incomplete"):
        collect(rows, lambda _: None, destination)
