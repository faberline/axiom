import itertools
import random
import ast

import pyarrow.parquet as pq
import pytest

from som.developer.data import check_cases,instantiate,REVIEW_ID
from som.developer.mbpp_cases import DATA as MBPP,fingerprint
from som.developer.ranker_data import DATA,verify
from som.developer.ranker_fixtures import fixtures
from som.developer.ranker_holdout import DATA as FINAL,rows as holdout_rows
from som.developer.ranker_input import check_request,prompt,stable_scores
from som.paths import ROOT,read_json,read_rows
from som.schema import Candidate


def test_v4_boundaries_and_count_balance():
    result=verify()
    assert result["counts"]=={"train":1848,"validation":140,"calibration":316,"seen":320}
    manifest=read_json(DATA/"manifest.json")
    mbpp=manifest["mbpp_ids"]
    assert [len(mbpp[s]) for s in ("train","validation","calibration")]==[110,19,19]
    assert not set(mbpp["train"])&(set(mbpp["validation"])|set(mbpp["calibration"]))
    assert not set(mbpp["validation"])&set(mbpp["calibration"])
    official={c["id"]:c["official_split"] for c in read_json(MBPP/"cases.json")}
    assert all(official[i]=="train" for i in mbpp["train"])
    assert all(official[i]=="validation" for s in ("validation","calibration") for i in mbpp[s])
    assert len({r["question"] for r in read_rows(DATA/"train.jsonl")})==3


def test_new_public_holdout_excludes_prior_test_and_development():
    rows=holdout_rows(); manifest=read_json(FINAL/"manifest.json")
    assert len(rows)==240 and len(set(manifest["problem_ids"]))==40
    assert not set(manifest["problem_ids"])&set(manifest["excluded_v3_ids"])
    development=[r for s in ("train","validation","calibration","seen") for r in read_rows(DATA/f"{s}.jsonl")]
    for key in ("id","state","family"):
        assert not {r[key] for r in rows}&{r[key] for r in development}
    for case in read_json(FINAL/"cases.json"):
        assert [o["passed"] for o in case["oracle"]]==[True,False,False]
        assert all(o["valid_syntax"] for o in case["oracle"])


def test_mbpp_normalized_code_has_no_public_overlap():
    public=pq.read_table(ROOT/"data/humanevalpack-v3/source/python.parquet").to_pylist()
    excluded={fingerprint(r["declaration"]+r[k]) for r in public for k in ("canonical_solution","buggy_solution")}
    cases=read_json(MBPP/"cases.json")
    signatures=[fingerprint(c["sources"][0]) for c in cases]
    assert len(set(signatures))==len(signatures)
    assert not set(signatures)&excluded
    for case in cases:
        assert [o["passed"] for o in case["oracle"]]==[True,False,False,False]
        assert all(o["valid_syntax"] for o in case["oracle"])
        assert all(source==ast.unparse(ast.parse(source)) for source in case['sources'])
    assert fingerprint('def add(x): return x+1')==fingerprint('def incr(y): return y+1')
    assert fingerprint('def add(x): return x+1')!=fingerprint('def incr(y): return y-1')


def test_frontend_new_families_have_real_failing_candidates():
    rng=random.Random(985)
    cases=[instantiate(f,999,rng) for _,f in fixtures()]
    result=check_cases(cases)
    assert len(result)==24 and all(v==[True,False,False,False] for v in result.values())


def test_browser_diagnostic_excludes_all_development_families():
    from som.developer.ranker_browser import DATA as BROWSER,rows,fixtures as browser_fixtures
    records=rows()
    assert len(records)==48 and len({r['family'] for r in records})==6
    used={r['family'] for s in ('train','validation','calibration','seen') for r in read_rows(DATA/f'{s}.jsonl')}
    assert not used&{r['family'] for r in records}
    rng=random.Random(552)
    cases=[instantiate(f,1000,rng) for f in browser_fixtures()]
    assert all(v==[True,False,False,False] for v in check_cases(cases).values())
    assert all(v==[True,False,False,False] for v in read_json(BROWSER/'outcomes.json').values())


def test_exact_ties_are_order_invariant_and_review_first():
    candidates=[Candidate('b','same'),Candidate('a','same'),Candidate(REVIEW_ID,'review')]
    scores=[0.,0.,0.]
    expected=dict(zip([c.id for c in candidates],stable_scores(candidates,scores)))
    assert max(expected,key=expected.get)==REVIEW_ID
    for order in itertools.permutations(range(3)):
        actual=dict(zip([candidates[i].id for i in order],stable_scores([candidates[i] for i in order],[scores[i] for i in order])))
        assert actual==expected
    near=[1e-9,0.,0.]
    assert max(range(3),key=lambda i:stable_scores(candidates,near)[i])==0


def test_candidate_prompt_has_no_id_or_position_and_rejects_bad_tokens():
    class Tokenizer:
        tokens=[1,2,3]
        def apply_chat_template(self,messages,**kwargs):
            self.messages=messages
            assert kwargs["return_dict"] is False and kwargs["enable_thinking"] is False
            return self.tokens
        def encode(self,text,**kwargs):
            return {'Yes':[4],'No':[5]}.get(text,list(range(len(text.split()))))
    t=Tokenizer()
    assert prompt('return one','choose','def f(): return 1',t)==([1,2,3],[4,5])
    assert 'def f(): return 1' in t.messages[1]["content"]
    t.tokens={'input_ids':[1]}
    with pytest.raises(ValueError,match='flat list'):
        prompt('s','q','c',t)
    t.tokens=[1]*1537
    with pytest.raises(ValueError,match='No text was truncated'):
        prompt('s','q','c',t)
    value={'state':'word '*1600,'question':'q','candidates':[{'id':'a','text':'x'},{'id':'b','text':'y'}]}
    with pytest.raises(ValueError,match='No text was truncated'):
        check_request(value,t)


def test_feature_transfer_keeps_only_identical_rows_and_charges_time(tmp_path,monkeypatch):
    from som.developer import ranker_cache as cache
    from som.paths import write_json,sha256
    monkeypatch.setattr(cache,'ROOT',tmp_path)
    source=tmp_path/'old'; destination=tmp_path/'new'; saved=source/'checkpoints/zero'
    write_json(source/'latest.json',{'checkpoint':'checkpoints/zero'})
    write_json(saved/'trainer.json',{'samples_seen':0,'updates':0,'status':'interrupted','elapsed_seconds':120.})
    code={}
    for name in cache.FEATURE_CODE:
        p=tmp_path/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text('frozen')
        code[name]=sha256(p)
    (tmp_path/'modern.sources.lock.json').write_text('{}')
    write_json(source/'acceptance-protocol.json',{'code_sha256':code,'source_sha256':sha256(tmp_path/'modern.sources.lock.json')})
    (source/'initial').mkdir();(source/'initial/adapter.safetensors').write_bytes(b'initial')
    old=[{'id':'unchanged','state':'one'},{'id':'changed','state':'two'}]
    (source/'inputs').mkdir();(source/'inputs/train.jsonl').write_text('\n'.join(__import__('json').dumps(r) for r in old))
    write_json(source/'inputs/manifest.json',{'sha256':{'train':sha256(source/'inputs/train.jsonl')}})
    new=[{'id':'changed','state':'different'},old[0]]
    (tmp_path/'data/ranker-v4').mkdir(parents=True)
    (tmp_path/'data/ranker-v4/train.jsonl').write_text('\n'.join(__import__('json').dumps(r) for r in new))
    folder=source/'features/train';folder.mkdir(parents=True);index={}
    for i,row in enumerate(old):
        name=f'{i:06d}.safetensors';(folder/name).write_bytes(str(i).encode())
        index[name]={'id':row['id'],'sha256':sha256(folder/name)}
    write_json(folder/'index.json',index)
    assert cache.transfer(source,destination)=={'transferred':1,'elapsed_seconds':120.}
    assert (destination/'features/train/000001.safetensors').read_bytes()==b'0'
    assert not (destination/'features/train/000000.safetensors').exists()
    assert cache.carried_seconds(destination)==120.
    (tmp_path/cache.FEATURE_CODE[0]).write_text('changed')
    with pytest.raises(ValueError,match='feature code changed'):
        cache.carried_seconds(destination)
