"""Shared Yes/No verifier with order-independent candidate scores."""
import json
import math
import time
from pathlib import Path

import mlx.core as mx
from mlx.utils import tree_flatten
from mlx_lm import load

from ..metrics import probabilities
from ..model import setup_memory
from ..paths import ROOT,read_json,resolve_checkpoint,sha256,write_json
from .modern_assets import MODEL_PATH,LOCK
from .modern_model import ModernDecision
from .ranker_input import MAX_TOKENS,check_request,prompt,stable_scores
from .data import REVIEW_ID

RUN=ROOT/"runs/som/research/ranker"
CONFIG={"architecture":"qwen35_independent_candidate_verifier","rank":8,"scale":16.,"seed":47,
        "max_tokens":MAX_TOKENS,"thinking":False,"review_logit":0.,
        "lora_layers":"last full-attention block; Q/V only; detached frozen prefix",
        "score":"Yes minus No logit; none-of-the-above has zero logit; exact ties prefer review then text and ID"}


def load_ranker(checkpoint=None):
    setup_memory(); mx.random.seed(CONFIG["seed"])
    base,tokenizer=load(str(MODEL_PATH),tokenizer_config={"local_files_only":True})
    model=ModernDecision(base); mx.eval(model.parameters())
    if checkpoint is not None:
        restore(model,checkpoint)
    model.eval(); return model,tokenizer


def save(model,destination):
    destination=Path(destination); destination.mkdir(parents=True,exist_ok=True)
    mx.save_safetensors(str(destination/"adapter.safetensors"),dict(tree_flatten(model.trainable_parameters())))
    write_json(destination/"model_config.json",{**CONFIG,"source_lock_sha256":sha256(LOCK),"source":read_json(LOCK)})


def restore(model,destination):
    destination=resolve_checkpoint(destination); cfg=read_json(destination/"model_config.json")
    if any(cfg.get(k)!=v for k,v in CONFIG.items()) or cfg["source_lock_sha256"]!=sha256(LOCK):
        raise ValueError("Independent verifier checkpoint configuration or source changed.")
    weights=mx.load(str(destination/"adapter.safetensors")); expected=dict(tree_flatten(model.trainable_parameters()))
    if weights.keys()!=expected.keys() or any(weights[k].shape!=expected[k].shape for k in expected):
        raise ValueError("Verifier adapter names or shapes changed.")
    model.load_weights(list(weights.items()),strict=False); mx.eval(model.parameters())


class Scorer:
    """Cache the last candidate set only. Reverse-order calls reuse identical scores."""
    def __init__(self,model,tokenizer):
        self.model=model; self.tokenizer=tokenizer; self._key=None; self._scores=None

    def score(self,value,use_cache=True):
        start=time.perf_counter(); r,size=check_request(value,self.tokenizer)
        canonical=sorted(r.candidates,key=lambda c:(c.text,c.id))
        key=json.dumps([r.state,r.question,[(c.id,c.text) for c in canonical]],ensure_ascii=False)
        hit=use_cache and key==self._key
        if hit:
            mapped=self._scores
        else:
            values=[]
            for c in canonical:
                if c.id==REVIEW_ID:
                    values.append(0.); continue
                tokens,labels=prompt(r.state,r.question,c.text,self.tokenizer)
                z=self.model(mx.array([tokens]),labels)[0]
                mx.eval(z); values.append(float((z[0]-z[1]).item()))
            if not all(math.isfinite(v) for v in values):
                raise RuntimeError("Non-finite verifier scores.")
            mapped={c.id:s for c,s in zip(canonical,stable_scores(canonical,values))}
            self._key=key; self._scores=mapped
        return {"logits":[mapped[c.id] for c in r.candidates],"seconds":time.perf_counter()-start,
                "input_tokens":size,"order_cache_hit":hit}

    def paired(self,row):
        start=time.perf_counter()
        first=self.score(row,use_cache=False)
        reverse={**row,"candidates":list(reversed(row["candidates"]))}
        second=self.score(reverse)
        return {**first,"reverse_logits":second["logits"],"reverse_candidate_ids":[c["id"] for c in reverse["candidates"]],
                "paired_seconds":time.perf_counter()-start,"reverse_cache_hit":second["order_cache_hit"]}


class RankerBasePredictor:
    def __init__(self,checkpoint):
        self.model,self.tokenizer=load_ranker(checkpoint); self.scorer=Scorer(self.model,self.tokenizer)
        self.max_tokens=MAX_TOKENS

    def token_count(self,value):
        return check_request(value,self.tokenizer)[1]

    def predict(self,value):
        request,_=check_request(value,self.tokenizer); result=self.scorer.score(value)
        p=probabilities(result["logits"]); best=int(p.argmax())
        return {"choice_id":request.candidates[best].id,"calibrated":False,
                "candidates":[{"id":c.id,"score":result["logits"][i],"probability":float(p[i])} for i,c in enumerate(request.candidates)]}
