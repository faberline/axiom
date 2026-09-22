import os
import itertools
import numpy as np
import pytest

pytestmark=pytest.mark.skipif(os.getenv('JEV_GPU_TESTS')!='1',reason='Explicit local GPU tests')


def test_ranker_gradients_resume_uncached_permutations_and_contract(tmp_path):
    import mlx.core as mx
    import mlx.nn as nn
    import mlx.optimizers as optim
    from mlx.utils import tree_flatten,tree_unflatten
    from som.developer.ranker_model import load_ranker,restore,Scorer,RankerBasePredictor
    from som.developer.ranker_loss import features_for_row,loss
    from som.developer.ranker_train import checkpoint,freeze,verify_protocol
    from som.developer.ranker_input import prompt
    from som.developer.data import REVIEW_ID,REVIEW_TEXT
    from som.paths import read_json,write_json,sha256
    import gc
    gc.collect(); mx.clear_cache(); mx.reset_peak_memory()
    model,tokenizer=load_ranker()
    params=dict(tree_flatten(model.trainable_parameters()))
    assert len(params)==4 and sum(p.size for p in params.values())==114688
    frozen=np.array(model.language_model.model.embed_tokens.weight)
    row={'id':'test','state':'Return x plus one.','question':'Choose the correct function.',
         'candidates':[{'id':'yes','text':'def f(x): return x+1'},{'id':'no','text':'def f(x): return x-1'},
                       {'id':REVIEW_ID,'text':REVIEW_TEXT}],'gold_candidate_id':'yes'}
    features=features_for_row(model,tokenizer,row)
    codes=sorted(row['candidates'][:2],key=lambda c:(c['text'],c['id']))
    tokens,labels=prompt(row['state'],row['question'],codes[0]['text'],tokenizer)
    stock=model.language_model(mx.array([tokens]))[0,-1,labels].astype(mx.float32)
    cached=model.score_features(features['hidden0'],features['labels'])[0]
    mx.eval(stock,cached)
    np.testing.assert_allclose(np.array(stock),np.array(cached),atol=1e-5,rtol=0)
    optimizer=optim.AdamW(learning_rate=1e-4,weight_decay=0); optimizer.init(model.trainable_parameters())
    gradient=nn.value_and_grad(model,loss)
    def step():
        model.train(); value,grads=gradient(model,features)
        optimizer.update(model,grads); mx.eval(value,model.parameters(),optimizer.state)
        assert np.isfinite(value.item())
        return value.item()
    before=step(); saved=checkpoint(model,optimizer,tmp_path,{'samples_seen':1}); after=step()
    assert after<before
    expected={k:np.array(v) for k,v in tree_flatten(model.trainable_parameters())}
    restore(model,saved); optimizer.state=tree_unflatten(list(mx.load(str(saved/'optimizer.npz')).items()))
    mx.random.state=[mx.load(str(saved/'rng.npz'))['state']]; step()
    for k,v in tree_flatten(model.trainable_parameters()):
        np.testing.assert_allclose(np.array(v),expected[k],atol=1e-7,rtol=0)
    np.testing.assert_array_equal(frozen,np.array(model.language_model.model.embed_tokens.weight))
    model.eval(); scorer=Scorer(model,tokenizer)
    expected=scorer.score(row,use_cache=False)['logits']
    for order in itertools.permutations(range(3)):
        value={**row,'candidates':[row['candidates'][i] for i in order]}
        actual=scorer.score(value,use_cache=False)
        assert actual['order_cache_hit'] is False
        assert actual['logits']==[expected[i] for i in order]
    paired=scorer.paired(row)
    assert paired['reverse_cache_hit'] and paired['logits']==paired['reverse_logits'][::-1]
    predictor=object.__new__(RankerBasePredictor)
    predictor.model=model; predictor.tokenizer=tokenizer; predictor.scorer=scorer; predictor.max_tokens=1536
    for n in range(2,9):
        value={**row,'candidates':[{'id':f'p{i}','text':f'def f(x): return x+{i}'} for i in range(n)]}
        result=predictor.predict(value)
        assert len(result['candidates'])==n
        assert sum(c['probability'] for c in result['candidates'])==pytest.approx(1,abs=1e-6)
        assert result['choice_id']==max(result['candidates'],key=lambda c:c['score'])['id']
    with pytest.raises(ValueError,match='unique'):
        predictor.predict({**row,'candidates':[row['candidates'][0]]*2})
    with pytest.raises(ValueError,match='No text was truncated'):
        predictor.predict({**row,'state':'word '*2000})
    from som.developer.predict import DeveloperPredictor
    write_json(saved/'developer_policy.json',{'adapter_sha256':sha256(saved/'adapter.safetensors'),
        'acceptance_passed':False,'validated_max_tokens':1536,'validated_candidate_counts':[3,4],
        'domains':{d:{'temperature':1.,'threshold':0.} for d in ('python','frontend')}})
    restore(model,saved); model.eval(); predictor.scorer=Scorer(model,tokenizer)
    wrapper=DeveloperPredictor(saved,base_factory=lambda _:predictor)
    value={**row,'candidates':row['candidates'][:2]}
    decision=wrapper.predict(value,'python')
    assert decision['status']=='human_review' and decision['choice_id'] is None
    assert 'developer_acceptance_gates_not_met' in decision['review_reasons']
    assert len(value['candidates'])==2
    freeze(tmp_path); verify_protocol(tmp_path)
    protocol=read_json(tmp_path/'acceptance-protocol.json'); protocol['criteria']['fresh_family_accuracy'][0]=.1
    write_json(tmp_path/'acceptance-protocol.json',protocol)
    with pytest.raises(ValueError,match='protocol'):
        verify_protocol(tmp_path)
    assert mx.get_peak_memory()/1024**3<24
