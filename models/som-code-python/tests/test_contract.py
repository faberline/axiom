import copy

import numpy as np
import pytest

from som.metrics import fit_temperature, probabilities, summarize
from som.schema import DecisionRequest, encode


class CharacterTokenizer:
    def encode(self, value, **kwargs):
        return list(value.encode())


def request(n=2):
    return {"state": "I lost my card.", "question": "Which issue?",
            "candidates": [{"id": f"id-{i}", "text": f"Option {i}"} for i in range(n)]}


@pytest.mark.parametrize("n", range(2, 9))
def test_candidate_counts_and_end_positions(n):
    value = request(n)
    encoded = encode(value, CharacterTokenizer(), max_tokens=4096)
    assert len(encoded.candidate_positions) == n
    for i, position in enumerate(encoded.candidate_positions):
        prefix = bytes(encoded.tokens[:position + 1]).decode()
        assert prefix.endswith('"\nEND CANDIDATE\n')
        assert value["candidates"][i]["text"] in prefix
        if i + 1 < n:
            assert value["candidates"][i + 1]["text"] not in prefix
    assert encoded.decision_position == len(encoded.tokens) - 1
    renamed = copy.deepcopy(value)
    for candidate in renamed["candidates"]:
        candidate["id"] = "different-" + candidate["id"]
    assert encode(renamed, CharacterTokenizer(), 4096) == encoded


@pytest.mark.parametrize("n", [0, 1, 9])
def test_reject_candidate_count(n):
    with pytest.raises(ValueError, match="2–8"):
        DecisionRequest.from_dict(request(n))


@pytest.mark.parametrize("field", ["id", "text"])
@pytest.mark.parametrize("bad", ["", "  ", None, 1])
def test_reject_invalid_candidate(field, bad):
    value = request()
    value["candidates"][0][field] = bad
    with pytest.raises(ValueError):
        DecisionRequest.from_dict(value)


def test_reject_duplicate_ids_and_overlong_input():
    value = request()
    value["candidates"][1]["id"] = value["candidates"][0]["id"]
    with pytest.raises(ValueError, match="unique"):
        DecisionRequest.from_dict(value)
    value = request()
    value["state"] = "long text " * 1000
    with pytest.raises(ValueError, match="No text was truncated"):
        encode(value, CharacterTokenizer())


def test_calibration_keeps_choices_and_probabilities():
    rows = [{"logits": [8, 0], "gold_index": i % 2, "seconds": .01} for i in range(20)]
    fitted = fit_temperature(rows)
    assert fitted["nll_after"] < fitted["nll_before"]
    assert fitted["temperature"] > 0
    for row in rows:
        p = probabilities(row["logits"], fitted["temperature"])
        assert p.sum() == pytest.approx(1)
        assert np.argmax(row["logits"]) == p.argmax()
    metrics = summarize(rows, fitted["temperature"])
    assert metrics["accuracy"] == .5
    assert metrics["risk_coverage"][-1]["error"] is None


def test_probability_error_against_known_uniform_case():
    rows = [{"logits": [0, 0, 0, 0], "gold_index": 0, "seconds": .02}]
    metrics = summarize(rows)
    assert metrics["nll"] == pytest.approx(np.log(4))
    assert metrics["brier"] == pytest.approx(.75)
    assert metrics["latency_ms"]["p50"] == 20
