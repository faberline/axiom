"""Run explicitly with JEV_GPU_TESTS=1 on a Mac with Metal access."""
import os

import numpy as np
import pytest

pytestmark = pytest.mark.skipif(os.getenv("JEV_GPU_TESTS") != "1", reason="Explicit local GPU tests")


def test_weights_resume_and_api(tmp_path):
    import mlx.core as mx
    import mlx.nn as nn
    import mlx.optimizers as optim
    from mlx.utils import tree_flatten, tree_unflatten
    from som.model import load_model, encoded_logits, restore_adapter, Predictor
    from som.paths import ROOT, read_rows
    from som.schema import encode
    from som.train import checkpoint

    model, tokenizer = load_model()
    trainable = dict(tree_flatten(model.trainable_parameters()))
    assert len(trainable) == 115
    assert sum(v.size for v in trainable.values()) == 1672192
    assert all(".lora_a" in k or ".lora_b" in k or k in {"key.weight", "query.weight", "local.weight"}
               for k in trainable)
    assert all(k.startswith("backbone.layers.") and (".q_proj." in k or ".v_proj." in k)
               for k in trainable if ".lora_" in k)
    frozen_before = np.array(model.backbone.layers[0].self_attn.q_proj.linear.weight)
    row = read_rows(ROOT / "data" / "train.jsonl")[0]
    encoded = encode(row, tokenizer)
    gold = next(i for i, c in enumerate(row["candidates"]) if c["id"] == row["gold_candidate_id"])
    optimizer = optim.AdamW(learning_rate=1e-4, weight_decay=0)
    optimizer.init(model.trainable_parameters())

    def loss(current):
        z = current(mx.array([encoded.tokens]), encoded.candidate_positions, encoded.decision_position)
        return nn.losses.cross_entropy(z, mx.array([gold]), reduction="mean")

    gradient = nn.value_and_grad(model, loss)
    def step():
        value, grads = gradient(model)
        optimizer.update(model, grads)
        mx.eval(value, model.parameters(), optimizer.state)

    step()
    saved = checkpoint(model, optimizer, tmp_path, {"samples_seen": 1})
    np.testing.assert_array_equal(frozen_before, np.array(model.backbone.layers[0].self_attn.q_proj.linear.weight))
    step()
    expected = {k: np.array(v) for k, v in tree_flatten(model.trainable_parameters())}
    restore_adapter(model, saved)
    optimizer.state = tree_unflatten(list(mx.load(str(saved / "optimizer.npz")).items()))
    mx.random.state = [mx.load(str(saved / "rng.npz"))["state"]]
    step()
    for k, value in tree_flatten(model.trainable_parameters()):
        np.testing.assert_allclose(np.array(value), expected[k], rtol=0, atol=1e-7)
    # The public API loads a saved adapter plus its initial base model.
    predictor = Predictor(tmp_path)
    for n in range(2, 9):
        value = {"state": "I feel happy.", "question": "Which feeling?",
                 "candidates": [{"id": f"custom-{i}", "text": f"emotion {i}"} for i in range(n)]}
        prediction = predictor.predict(value)
        assert prediction["choice_id"] in {c["id"] for c in value["candidates"]}
        assert prediction["choice_id"] == max(prediction["candidates"], key=lambda c: c["score"])["id"]
        assert [c["id"] for c in prediction["candidates"]] == [c["id"] for c in value["candidates"]]
        assert sum(c["probability"] for c in prediction["candidates"]) == pytest.approx(1, abs=1e-6)
        assert all(np.isfinite(c["score"]) and 0 <= c["probability"] <= 1 for c in prediction["candidates"])
        renamed = {**value, "candidates": [{**c, "id": f"new-{i}"} for i, c in enumerate(value["candidates"])]}
        other = predictor.predict(renamed)
        assert [c["score"] for c in other["candidates"]] == [c["score"] for c in prediction["candidates"]]
        assert other["choice_id"] == "new-" + prediction["choice_id"].split("-")[-1]


def test_developer_api_review_and_length(tmp_path):
    from som.developer.train import CONFIG
    from som.developer.predict import DeveloperPredictor
    from som.model import load_model, save_adapter
    from som.paths import write_json, sha256
    model, _ = load_model(CONFIG)
    save_adapter(model, tmp_path)
    policy = {"adapter_sha256": sha256(tmp_path / "adapter.safetensors"), "acceptance_passed": False,
              "validated_max_tokens": 325,
              "domains": {d:{"temperature":1.0,"threshold":None} for d in ("python","frontend")}}
    write_json(tmp_path / "developer_policy.json", policy)
    predictor = DeveloperPredictor(tmp_path)
    request = {"state":"Select a valid update.","candidates":[
        {"id":"a","text":"return x + 1"},{"id":"b","text":"return x - 1"}]}
    result = predictor.predict(request,"python")
    assert result["status"] == "human_review"
    assert result["choice_id"] is None
    assert "developer_acceptance_gates_not_met" in result["review_reasons"]
    assert len(request["candidates"]) == 2
    assert sum(c["probability"] for c in result["candidates"]) == pytest.approx(1)
    with pytest.raises(ValueError,match="Domain"):
        predictor.predict(request,"unknown")
    with pytest.raises(ValueError,match="JSON object"):
        predictor.predict([],"python")
    long_request = {**request,"state":"word " * 600}
    long_result = predictor.predict(long_request,"python")
    assert long_result["input_tokens"] > 512
    assert "input_longer_than_validated_range" in long_result["review_reasons"]
    with pytest.raises(ValueError,match="No text was truncated"):
        predictor.predict({**request,"state":"word " * 1500},"python")
    policy["domains"]["python"]["temperature"] = 0
    write_json(tmp_path / "developer_policy.json",policy)
    with pytest.raises(ValueError,match="temperature"):
        DeveloperPredictor(tmp_path)
