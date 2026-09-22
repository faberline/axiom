"""Direct candidate scores from Qwen3.5, with thinking explicitly disabled."""
import math
import time
from pathlib import Path

import mlx.core as mx
import mlx.nn as nn
from mlx.utils import tree_flatten
from mlx_lm import load
from mlx_lm.tuner.lora import LoRALinear
from mlx_lm.models.base import create_attention_mask, create_ssm_mask

from ..model import setup_memory
from ..paths import read_json, resolve_checkpoint, sha256, write_json
from ..schema import DecisionRequest
from .modern_assets import MODEL_PATH, LOCK
from .modern_input import prompt
from ..specialists.score_contract import combine_candidate_and_prompt_logits

CONFIG={"architecture":"qwen35_direct_selector","rank":8,"scale":16.0,"seed":45,
        "max_tokens":1536,"thinking":False,"lora_layers":"last full-attention block; Q/V only; detached frozen prefix"}



class ModernDecision(nn.Module):
    def __init__(self,base):
        super().__init__()
        self.language_model=base.language_model
        self.language_model.freeze()
        count=0
        layers=self.language_model.model.layers
        if len(layers)!=32 or layers[-1].is_linear:
            raise ValueError("Expected the pinned 32-layer model ending in full attention.")
        for index,layer in enumerate(layers):
            if index!=len(layers)-1:
                continue
            count+=1
            for name in ("q_proj","v_proj"):
                lora=LoRALinear.from_base(getattr(layer.self_attn,name),r=8,scale=16,dropout=0)
                lora.linear.freeze()
                setattr(layer.self_attn,name,lora)
        if count!=1:
            raise ValueError("Expected exactly one trainable attention block.")

    def train(self,mode=True):
        super().train(mode)
        for layer in self.language_model.model.layers[:-1]:
            layer.eval()
        return self

    def prefix_features(self,tokens):
        backbone=self.language_model.model
        hidden=backbone.embed_tokens(tokens)
        attention_mask=create_attention_mask(hidden,None)
        linear_mask=create_ssm_mask(hidden,None)
        for layer in backbone.layers[:-1]:
            hidden=layer(hidden,mask=linear_mask if layer.is_linear else attention_mask,cache=None)
        return mx.stop_gradient(hidden)

    def score_features(self,hidden,labels,candidate_end=None):
        """Use frozen Yes/No logits at prompt end and, when supplied, boundary.

        ``candidate_end=None`` preserves the former final-position-only API
        for older research callers.  SOM passes its controlled boundary index.
        """
        backbone=self.language_model.model
        attention_mask=create_attention_mask(hidden,None)
        hidden=backbone.layers[-1](hidden,mask=attention_mask,cache=None)
        def project(position):
            value=backbone.norm(hidden[:,position,:])
            return (self.language_model.model.embed_tokens.as_linear(value) if self.language_model.args.tie_word_embeddings
                    else self.language_model.lm_head(value))[:,labels].astype(mx.float32)
        final=project(-1)
        if candidate_end is None:
            return final
        position=int(candidate_end.item()) if hasattr(candidate_end,"item") else int(candidate_end)
        if not 0 <= position < hidden.shape[1]:
            raise ValueError("Candidate boundary position is outside cached hidden states.")
        return combine_candidate_and_prompt_logits(project(position),final)

    def __call__(self,tokens,labels,candidate_end=None):
        return self.score_features(self.prefix_features(tokens),labels,candidate_end)


def load_modern(checkpoint=None):
    setup_memory()
    mx.random.seed(CONFIG["seed"])
    base,tokenizer=load(str(MODEL_PATH),tokenizer_config={"local_files_only":True})
    model=ModernDecision(base)
    mx.eval(model.parameters())
    if checkpoint is not None:
        restore(model,checkpoint)
    model.eval()
    return model,tokenizer


def score(model,tokenizer,row):
    start=time.perf_counter()
    tokens,labels=prompt(row,tokenizer)
    z=model(mx.array([tokens]),labels)[0]
    mx.eval(z)
    values=z.tolist()
    if not all(math.isfinite(x) for x in values):
        raise RuntimeError("Non-finite modern model scores.")
    return {"logits":values,"seconds":time.perf_counter()-start,"input_tokens":len(tokens)}


def paired_score(model,tokenizer,row):
    start=time.perf_counter()
    first=score(model,tokenizer,row)
    reversed_row={**row,"candidates":list(reversed(row["candidates"]))}
    other=score(model,tokenizer,reversed_row)
    return {**first,"reverse_logits":other["logits"],
            "reverse_candidate_ids":[c["id"] for c in reversed_row["candidates"]],
            "paired_seconds":time.perf_counter()-start}


def save(model,destination):
    destination=Path(destination)
    destination.mkdir(parents=True,exist_ok=True)
    mx.save_safetensors(str(destination/"adapter.safetensors"),dict(tree_flatten(model.trainable_parameters())))
    write_json(destination/"model_config.json",{**CONFIG,"source_lock_sha256":sha256(LOCK),"source":read_json(LOCK)})


def restore(model,destination):
    destination=resolve_checkpoint(destination)
    cfg=read_json(destination/"model_config.json")
    if any(cfg.get(k)!=v for k,v in CONFIG.items()) or cfg["source_lock_sha256"]!=sha256(LOCK):
        raise ValueError("Modern checkpoint configuration or source changed.")
    weights=mx.load(str(destination/"adapter.safetensors"))
    expected=dict(tree_flatten(model.trainable_parameters()))
    if weights.keys()!=expected.keys() or any(weights[k].shape!=expected[k].shape for k in expected):
        raise ValueError("Modern adapter names or shapes changed.")
    model.load_weights(list(weights.items()),strict=False)
    mx.eval(model.parameters())


class ModernBasePredictor:
    def __init__(self,checkpoint):
        self.model,self.tokenizer=load_modern(checkpoint)
        self.max_tokens=CONFIG["max_tokens"]

    def token_count(self,value):
        return len(prompt(value,self.tokenizer)[0])

    def predict(self,value):
        r=DecisionRequest.from_dict(value)
        result=score(self.model,self.tokenizer,value)
        z=mx.array(result["logits"]); p=mx.softmax(z); mx.eval(p)
        return {"choice_id":r.candidates[int(z.argmax().item())].id,"calibrated":False,
                "candidates":[{"id":c.id,"score":result["logits"][i],"probability":float(p[i].item())}
                              for i,c in enumerate(r.candidates)]}
