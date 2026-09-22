"""Use the coder model's pretrained letter logits, with optional Q/V LoRA."""
import json
import math
import time
from pathlib import Path

import mlx.core as mx
import mlx.nn as nn
from mlx.utils import tree_flatten
from mlx_lm import load
from mlx_lm.tuner.lora import LoRALinear

from ..model import setup_memory
from ..paths import ROOT, read_json, resolve_checkpoint, sha256, write_json
from ..schema import DecisionRequest
from .coder_assets import MODEL_PATH

CONFIG = {"architecture":"qwen25_coder_letter_selector", "rank":8, "scale":16.0, "seed":44, "max_tokens":1536}


def prompt(value, tokenizer):
    request = DecisionRequest.from_dict(value)
    options = "\n\n".join(f"{chr(65+i)}: {c.text}" for i,c in enumerate(request.candidates))
    user = f"STATE:\n{request.state}\n\nQUESTION:\n{request.question}\n\nCANDIDATES:\n{options}\n\nReturn only the letter of the best candidate."
    tokens = tokenizer.apply_chat_template([
        {"role":"system","content":"You are a careful Python and JavaScript code reviewer. Choose the replacement that satisfies every stated requirement. If none satisfies the requirement, choose the human review option when supplied. Return one candidate letter only."},
        {"role":"user","content":user}],tokenize=True,add_generation_prompt=True)
    if len(tokens)>CONFIG["max_tokens"]:
        raise ValueError(f"Input has {len(tokens)} tokens; maximum is {CONFIG['max_tokens']}. No text was truncated.")
    labels=[tokenizer.encode(chr(65+i),add_special_tokens=False) for i in range(len(request.candidates))]
    if any(len(x)!=1 for x in labels):
        raise ValueError("Coder selector needs single-token letter labels.")
    return tokens,[x[0] for x in labels]


class CoderDecision(nn.Module):
    def __init__(self,language_model):
        super().__init__()
        self.language_model=language_model
        language_model.freeze()
        for layer in language_model.model.layers:
            for name in ("q_proj","v_proj"):
                adapter=LoRALinear.from_base(getattr(layer.self_attn,name),r=CONFIG["rank"],scale=CONFIG["scale"],dropout=0)
                adapter.linear.freeze()
                setattr(layer.self_attn,name,adapter)

    def __call__(self,tokens,labels):
        hidden=self.language_model.model(tokens)[:,-1,:]
        logits=(self.language_model.model.embed_tokens.as_linear(hidden) if self.language_model.args.tie_word_embeddings
                else self.language_model.lm_head(hidden))
        return logits[:,labels].astype(mx.float32)


def load_coder(checkpoint=None):
    setup_memory()
    mx.random.seed(CONFIG["seed"])
    base,tokenizer=load(str(MODEL_PATH),tokenizer_config={"local_files_only":True})
    model=CoderDecision(base)
    mx.eval(model.parameters())
    if checkpoint is not None:
        restore(model,checkpoint)
    model.eval()
    return model,tokenizer


def score(model,tokenizer,row):
    start=time.perf_counter()
    tokens,labels=prompt(row,tokenizer)
    logits=model(mx.array([tokens]),labels)[0]
    mx.eval(logits)
    values=logits.tolist()
    if not all(math.isfinite(x) for x in values):
        raise RuntimeError("Coder returned non-finite logits.")
    return {"logits":values,"seconds":time.perf_counter()-start,"input_tokens":len(tokens)}


def paired_score(model,tokenizer,row):
    start=time.perf_counter()
    first=score(model,tokenizer,row)
    reverse={**row,"candidates":list(reversed(row["candidates"]))}
    second=score(model,tokenizer,reverse)
    return {**first,"reverse_logits":second["logits"],"reverse_candidate_ids":[c["id"] for c in reverse["candidates"]],
            "paired_seconds":time.perf_counter()-start}


def save(model,path):
    path=Path(path)
    path.mkdir(parents=True,exist_ok=True)
    mx.save_safetensors(str(path/"adapter.safetensors"),dict(tree_flatten(model.trainable_parameters())))
    write_json(path/"model_config.json",{**CONFIG,"source":read_json(ROOT/"som-research-coder.sources.lock.json"),
               "source_lock_sha256":sha256(ROOT/"som-research-coder.sources.lock.json")})


def restore(model,path):
    path=resolve_checkpoint(path)
    config=read_json(path/"model_config.json")
    if any(config.get(k)!=v for k,v in CONFIG.items()) or config["source_lock_sha256"]!=sha256(ROOT/"som-research-coder.sources.lock.json"):
        raise ValueError("Coder checkpoint configuration or source changed.")
    weights=mx.load(str(path/"adapter.safetensors"))
    expected=dict(tree_flatten(model.trainable_parameters()))
    if weights.keys()!=expected.keys() or any(weights[k].shape!=expected[k].shape for k in expected):
        raise ValueError("Coder adapter names or shapes changed.")
    model.load_weights(list(weights.items()),strict=False)
    mx.eval(model.parameters())


class CoderBasePredictor:
    def __init__(self,checkpoint):
        self.model,self.tokenizer=load_coder(checkpoint)
        self.max_tokens=CONFIG["max_tokens"]

    def token_count(self,value):
        return len(prompt(value,self.tokenizer)[0])

    def predict(self,value):
        request=DecisionRequest.from_dict(value)
        result=score(self.model,self.tokenizer,value)
        logits=mx.array(result["logits"])
        p=mx.softmax(logits)
        mx.eval(p)
        return {"choice_id":request.candidates[int(logits.argmax().item())].id,
                "candidates":[{"id":c.id,"score":result["logits"][i],"probability":float(p[i].item())}
                              for i,c in enumerate(request.candidates)],"calibrated":False}
