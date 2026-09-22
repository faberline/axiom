import pytest

from som.developer.data import DATA, PROTOCOL, REVIEW_ID, verify
from som.developer.fixtures import fixtures
from som.developer.policy import choose_threshold, decision, selective_metrics, wilson_upper
from som.paths import read_json, read_rows, sha256


def record(correct=True, stable=True, review=False):
    return {"candidate_ids": ["patch-a", "patch-b", REVIEW_ID],
            "logits": [8,0,-2] if not review else [-2,0,8],
            "reverse_logits": [-2,0,8] if stable and not review else [8,0,-2],
            "reverse_candidate_ids": [REVIEW_ID,"patch-b","patch-a"],
            "gold_index": (2 if review else 0) if correct else 1}


def test_wilson_bound_requires_evidence():
    assert wilson_upper(0,0) == 1
    assert wilson_upper(0,3) > .5
    assert .03 < wilson_upper(0,100) < .04
    assert wilson_upper(10,100) > .10


def test_calibration_cannot_accept_tiny_or_wrong_samples():
    assert choose_threshold([record() for _ in range(10)],1)["selected"]["threshold"] is None
    assert choose_threshold([record(correct=False) for _ in range(100)],1)["selected"]["threshold"] is None
    good = choose_threshold([record() for _ in range(100)],1)["selected"]
    assert good["accepted"] == 100
    assert good["threshold"] == .99
    assert good["error_wilson_upper_95"] < .10


def test_order_change_or_review_excludes_acceptance():
    result = selective_metrics([record(stable=False),record(review=True)],1,0)
    assert result["accepted"] == 0
    assert result["missing_fix_review_recall"] == 1
    assert decision(record(stable=False),1)["stable"] is False


@pytest.mark.skipif(not (DATA/"manifest.json").exists(), reason="Developer data has not been prepared")
def test_developer_data_boundaries_and_executed_labels():
    result = verify()
    assert result["unique_instances"] == 2528
    assert result["training_families"] == 32
    assert result["held_out_families"] == 8
    outcomes = read_json(DATA / "oracle-results.json")
    assert len(outcomes) == 2528
    assert all(v == [True,False,False,False] for v in outcomes.values())
    for split in ("calibration", "test", "challenge"):
        rows=read_rows(DATA / f"{split}.jsonl")
        assert sum(r["missing_correct_patch"] for r in rows)/len(rows) == .3
    assert not ({f.name for f in fixtures() if f.held_out} & {f.name for f in fixtures() if not f.held_out})


def test_developer_gate_protocol_is_not_the_first_round():
    assert PROTOCOL["max_tokens"] == 1024
    assert PROTOCOL["gates"]["held_out_accuracy_each_domain"] == .8
    assert PROTOCOL["synthetic_only"] is True
