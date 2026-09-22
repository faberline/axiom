"""Explicit GPU integration checks. Run after training, outside latency measurement."""
import os

import numpy as np
import pytest

pytestmark = pytest.mark.skipif(os.getenv("JEV_GPU_TESTS") != "1", reason="Explicit local GPU tests")


def test_coder_update_resume_and_candidate_contract(tmp_path):
    import mlx.core as mx
    import mlx.nn as nn
    import mlx.optimizers as optim
    from mlx.utils import tree_flatten, tree_unflatten
    from som.developer.coder_model import CoderBasePredictor, load_coder, prompt, restore, score
    from som.developer.coder_train import checkpoint
    from som.paths import write_json, sha256

    model, tokenizer = load_coder()
    trainable = dict(tree_flatten(model.trainable_parameters()))
    assert len(trainable) == 112
    assert sum(v.size for v in trainable.values()) == 2523136
    assert all((".q_proj." in k or ".v_proj." in k) and (k.endswith(".lora_a") or k.endswith(".lora_b")) for k in trainable)
    frozen = np.array(model.language_model.model.layers[0].self_attn.q_proj.linear.weight)
    row = {"state": "Return the value plus one.", "question": "Which function meets the requirement?",
           "candidates": [{"id": "one", "text": "def f(x): return x + 1"}, {"id": "two", "text": "def f(x): return x - 1"}]}
    tokens, labels = prompt(row, tokenizer)
    prior = np.asarray(score(model, tokenizer, row)["logits"], dtype=np.float32)
    optimizer = optim.AdamW(learning_rate=2e-5, weight_decay=0)
    optimizer.init(model.trainable_parameters())
    def loss(current):
        z = current(mx.array([tokens]), labels)[0]
        lp = z - mx.logsumexp(z)
        t = mx.array(prior)
        lt = t - mx.logsumexp(t)
        return -lp[0] + .5 * mx.sum(mx.exp(lt) * (lt - lp))
    gradient = nn.value_and_grad(model, loss)
    def step():
        value, grads = gradient(model)
        optimizer.update(model, grads)
        mx.eval(value, model.parameters(), optimizer.state)
        assert np.isfinite(value.item())
    step()
    saved = checkpoint(model, optimizer, tmp_path, {"samples_seen": 1})
    step()
    expected = {k: np.array(v) for k, v in tree_flatten(model.trainable_parameters())}
    restore(model, saved)
    optimizer.state = tree_unflatten(list(mx.load(str(saved / "optimizer.npz")).items()))
    mx.random.state = [mx.load(str(saved / "rng.npz"))["state"]]
    step()
    for k, v in tree_flatten(model.trainable_parameters()):
        np.testing.assert_allclose(np.array(v), expected[k], atol=1e-7, rtol=0)
    np.testing.assert_array_equal(frozen, np.array(model.language_model.model.layers[0].self_attn.q_proj.linear.weight))
    restore(model, saved)
    before = score(model, tokenizer, row)["logits"]
    predictor = CoderBasePredictor(tmp_path)
    after = predictor.predict(row)
    np.testing.assert_allclose([c["score"] for c in after["candidates"]], before, atol=1e-5, rtol=0)
    for n in range(2, 9):
        value = {**row, "candidates": [{"id": f"id-{i}", "text": f"def f(x): return x + {i}"} for i in range(n)]}
        result = predictor.predict(value)
        assert len(result["candidates"]) == n
        assert sum(c["probability"] for c in result["candidates"]) == pytest.approx(1, abs=1e-6)
        assert result["choice_id"] == max(result["candidates"], key=lambda c: c["score"])["id"]
        renamed = {**value, "candidates": [{**c, "id": f"new-{i}"} for i,c in enumerate(value["candidates"])]}
        other = predictor.predict(renamed)
        assert [c["score"] for c in result["candidates"]] == [c["score"] for c in other["candidates"]]
        assert other["choice_id"] == result["choice_id"].replace("id-", "new-")
    with pytest.raises(ValueError, match="No text was truncated"):
        predictor.predict({**row, "state": "word " * 2000})
    duplicate = {**row, "candidates": [row["candidates"][0]] * 2}
    with pytest.raises(ValueError, match="unique"):
        predictor.predict(duplicate)
    bad = {**row, "candidates": [{"id": "x", "text": " "}, row["candidates"][1]]}
    with pytest.raises(ValueError, match="nonempty"):
        predictor.predict(bad)
    from som.developer.coder_predict import CoderPredictor
    write_json(saved / "developer_policy.json", {
        "adapter_sha256": sha256(saved / "adapter.safetensors"), "acceptance_passed": False,
        "validated_max_tokens": 1536,
        "validated_candidate_counts": [3, 4, 5],
        "domains": {d:{"temperature":1., "threshold":None} for d in ("python", "frontend")}})
    wrapped = CoderPredictor(tmp_path)
    result = wrapped.predict(row, "python")
    assert result["status"] == "human_review"
    assert result["choice_id"] is None
    assert "developer_acceptance_gates_not_met" in result["review_reasons"]
    assert sum(c["probability"] for c in result["candidates"]) == pytest.approx(1, abs=1e-6)
    assert len(row["candidates"]) == 2
    with pytest.raises(ValueError, match="reserved"):
        wrapped.predict({**row, "candidates":[{"id":"__review__", "text":"x"}, row["candidates"][0]]}, "frontend")
    larger = {**row, "candidates":[{"id":str(i), "text":f"def f(x): return x + {i}"} for i in range(5)]}
    assert "candidate_count_outside_validated_range" in wrapped.predict(larger, "python")["review_reasons"]
