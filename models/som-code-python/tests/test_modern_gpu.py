import os

import numpy as np
import pytest

pytestmark=pytest.mark.skipif(os.getenv("JEV_GPU_TESTS")!="1",reason="Explicit local GPU tests")


def test_modern_cached_gradient_resume_and_contract(tmp_path):
    import mlx.core as mx
    import mlx.nn as nn
    import mlx.optimizers as optim
    from mlx.utils import tree_flatten,tree_unflatten
    from som.developer.modern_model import load_modern,score,restore,ModernBasePredictor
    from som.developer.modern_input import prompt
    from som.developer.modern_train import make_features,load_features,loss,checkpoint,freeze,verify_protocol
    from som.paths import read_json,write_json,sha256
    import gc
    gc.collect(); mx.clear_cache(); mx.reset_peak_memory()
    model,tokenizer=load_modern()
    parameters=dict(tree_flatten(model.trainable_parameters()))
    assert len(parameters)==4 and sum(v.size for v in parameters.values())==114688
    assert all("layers.31.self_attn." in k and ("q_proj" in k or "v_proj" in k) for k in parameters)
    frozen=np.array(model.language_model.model.embed_tokens.weight)
    row={"id":"cache-test","state":"Return x plus one.","question":"Choose the function.",
         "candidates":[{"id":"a","text":"def f(x): return x+1"},{"id":"b","text":"def f(x): return x-1"}],"gold_candidate_id":"a"}
    tokens,labels=prompt(row,tokenizer)
    stock=model.language_model(mx.array([tokens]))[0,-1,labels].astype(mx.float32)
    direct=model(mx.array([tokens]),labels)[0]
    mx.eval(stock,direct)
    np.testing.assert_allclose(np.array(stock),np.array(direct),atol=1e-5,rtol=0)
    cache=tmp_path/"features"
    assert make_features(model,tokenizer,[row],cache,lambda:False)
    features=load_features(cache,0)
    cached=model.score_features(features["hidden0"],features["labels0"])[0]
    mx.eval(cached)
    np.testing.assert_array_equal(np.array(direct),np.array(cached))
    optimizer=optim.AdamW(learning_rate=1e-4,weight_decay=0); optimizer.init(model.trainable_parameters())
    gradient=nn.value_and_grad(model,loss)
    def step():
        model.train(); value,grads=gradient(model,features,0)
        optimizer.update(model,grads); mx.eval(value,model.parameters(),optimizer.state)
        assert np.isfinite(value.item())
        return value.item()
    before=step(); saved=checkpoint(model,optimizer,tmp_path,{"samples_seen":1})
    after=step()
    assert after<before
    expected={k:np.array(v) for k,v in tree_flatten(model.trainable_parameters())}
    restore(model,saved)
    optimizer.state=tree_unflatten(list(mx.load(str(saved/"optimizer.npz")).items()))
    mx.random.state=[mx.load(str(saved/"rng.npz"))["state"]]
    step()
    for k,v in tree_flatten(model.trainable_parameters()):
        np.testing.assert_allclose(np.array(v),expected[k],atol=1e-7,rtol=0)
    np.testing.assert_array_equal(frozen,np.array(model.language_model.model.embed_tokens.weight))
    model.eval()
    predictor=object.__new__(ModernBasePredictor); predictor.model=model; predictor.tokenizer=tokenizer; predictor.max_tokens=1536
    for n in range(2,9):
        value={**row,"candidates":[{"id":f"id-{i}","text":f"def f(x): return x+{i}"} for i in range(n)]}
        result=predictor.predict(value)
        assert len(result["candidates"])==n
        assert sum(c["probability"] for c in result["candidates"])==pytest.approx(1,abs=1e-6)
        assert result["choice_id"]==max(result["candidates"],key=lambda c:c["score"])["id"]
        if n in (2,8):
            renamed={**value,"candidates":[{**c,"id":c["id"].replace("id-","new-")} for c in value["candidates"]]}
            other=predictor.predict(renamed)
            assert [c["score"] for c in other["candidates"]]==[c["score"] for c in result["candidates"]]
            assert other["choice_id"]==result["choice_id"].replace("id-","new-")
    with pytest.raises(ValueError,match="unique"):
        predictor.predict({**row,"candidates":[row["candidates"][0]]*2})
    with pytest.raises(ValueError,match="No text was truncated"):
        predictor.predict({**row,"state":"word "*2000})
    from som.developer.predict import DeveloperPredictor
    write_json(saved/"developer_policy.json",{"adapter_sha256":sha256(saved/"adapter.safetensors"),
        "acceptance_passed":False,"validated_max_tokens":1536,"validated_candidate_counts":[3,4],
        "domains":{d:{"temperature":1.,"threshold":0.} for d in ("python","frontend")}})
    restore(model,saved); model.eval()
    wrapper=DeveloperPredictor(saved,base_factory=lambda _:predictor)
    decision=wrapper.predict(row,"python")
    assert decision["status"]=="human_review" and decision["choice_id"] is None
    assert "developer_acceptance_gates_not_met" in decision["review_reasons"]
    assert len(row["candidates"])==2
    larger={**row,"candidates":[{"id":str(i),"text":f"def f(x): return x+{i}"} for i in range(4)]}
    assert "candidate_count_outside_validated_range" in wrapper.predict(larger,"python")["review_reasons"]
    freeze(tmp_path); verify_protocol(tmp_path)
    protocol=read_json(tmp_path/"acceptance-protocol.json"); protocol["criteria"]["fresh_family_accuracy"][0]=.1
    write_json(tmp_path/"acceptance-protocol.json",protocol)
    with pytest.raises(ValueError,match="protocol"):
        verify_protocol(tmp_path)
    assert mx.get_peak_memory()/1024**3<24
