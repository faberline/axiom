import os
import random
import numpy as np
import pytest

pytestmark=pytest.mark.skipif(os.getenv('JEV_GPU_TESTS')!='1',reason='Explicit local GPU tests')


def test_next_loss_rejection_and_stability():
    import mlx.core as mx
    from som.developer.next_loss import score_loss
    assert score_loss(mx.array([2.,-2.]),0).item()<score_loss(mx.array([2.,2.]),0).item()
    assert score_loss(mx.array([-2.,-2.]),2).item()<score_loss(mx.array([2.,2.]),2).item()
    assert score_loss(mx.array([2.,-2.]),0).item()<score_loss(mx.array([-2.,-2.]),0).item()
    for gold in (0,2):
        values=mx.array([1000.,-1000.]); loss,gradient=mx.value_and_grad(lambda v:score_loss(v,gold))(values)
        mx.eval(loss,gradient)
        assert np.isfinite(loss.item()) and np.isfinite(np.array(gradient)).all()


def test_next_32_row_smoke_exact_resume_and_frozen_prefix(tmp_path):
    import gc
    import mlx.core as mx
    import mlx.nn as nn
    import mlx.optimizers as optim
    from mlx.utils import tree_flatten,tree_map,tree_unflatten
    from som.paths import ROOT,read_rows,read_json,write_json,sha256
    from som.developer.next_train import START,START_SHA,PREVIOUS,verify_features,freeze,verify_protocol,TRAIN_CONFIG
    from som.developer.ranker_model import load_ranker,restore
    from som.developer.ranker_train import checkpoint,load_features
    from som.developer.ranker_input import prompt
    from som.developer.next_loss import loss
    gc.collect(); mx.clear_cache(); mx.reset_peak_memory()
    assert verify_features()=={'train':1848,'validation':140}
    assert sha256(START/'adapter.safetensors')==START_SHA
    model,tokenizer=load_ranker(START)
    params=dict(tree_flatten(model.trainable_parameters()))
    assert len(params)==4 and sum(v.size for v in params.values())==114688
    rows=read_rows(ROOT/'data/ranker-v4/train.jsonl')
    order=list(range(len(rows)));random.Random(TRAIN_CONFIG['seed']).shuffle(order)
    sample=order[:32]; assert {rows[i]['task'] for i in sample}=={'python','frontend'}
    directory=PREVIOUS/'features/train'
    def mean_loss():
        model.eval(); result=mx.stack([loss(model,load_features(directory,i)) for i in sample]).mean()
        mx.eval(result); return result.item()
    def prefix_check(i):
        row=rows[i]; codes=sorted([c for c in row['candidates'] if c['id']!='__review__'],key=lambda c:(c['text'],c['id']))
        tokens,labels=prompt(row['state'],row['question'],codes[0]['text'],tokenizer)
        f=load_features(directory,i)
        hidden=model.prefix_features(mx.array([tokens]));mx.eval(hidden)
        np.testing.assert_array_equal(np.array(hidden.astype(mx.float32)),np.array(f['hidden0'].astype(mx.float32)))
        direct=model(mx.array([tokens]),labels)
        cached=model.score_features(f['hidden0'],f['labels']);mx.eval(direct,cached)
        np.testing.assert_allclose(np.array(direct),np.array(cached),atol=1e-5,rtol=0)
    prefix_check(sample[0])
    optimizer=optim.AdamW(learning_rate=TRAIN_CONFIG['learning_rate'],weight_decay=0)
    optimizer.init(model.trainable_parameters());gradient=nn.value_and_grad(model,loss)
    def step(indices):
        model.train();acc=None
        for i in indices:
            value,grads=gradient(model,load_features(directory,i))
            acc=grads if acc is None else tree_map(lambda a,b:a+b,acc,grads)
            mx.eval(value,acc);assert np.isfinite(value.item())
        grads,norm=optim.clip_grad_norm(tree_map(lambda x:x/len(indices),acc),1.)
        optimizer.update(model,grads);mx.eval(model.parameters(),optimizer.state,norm)
    before=mean_loss()
    for i in range(0,32,8):
        step(sample[i:i+8])
    after=mean_loss();assert after<before
    saved=checkpoint(model,optimizer,tmp_path,{'samples_seen':32})
    step(order[32:40]);expected={k:np.array(v) for k,v in tree_flatten(model.trainable_parameters())}
    restore(model,saved);optimizer.state=tree_unflatten(list(mx.load(str(saved/'optimizer.npz')).items()))
    mx.random.state=[mx.load(str(saved/'rng.npz'))['state']]
    step(order[32:40])
    for k,v in tree_flatten(model.trainable_parameters()):
        np.testing.assert_allclose(np.array(v),expected[k],atol=1e-7,rtol=0)
    model.eval();prefix_check(sample[0])
    freeze(tmp_path);verify_protocol(tmp_path)
    value=read_json(tmp_path/'acceptance-protocol.json');value['training']['negative_margin']=0
    write_json(tmp_path/'acceptance-protocol.json',value)
    with pytest.raises(ValueError,match='protocol'):
        verify_protocol(tmp_path)
    assert mx.get_peak_memory()/1024**3<24
    print({'smoke_rows':32,'loss_before':before,'loss_after':after,'peak_mlx_gib':mx.get_peak_memory()/1024**3},flush=True)
