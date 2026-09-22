"""Small KL-regularized QLoRA run, retaining the pretrained decision head."""
import json
import random
import signal
import time
from pathlib import Path

import mlx.core as mx
import mlx.nn as nn
import mlx.optimizers as optim
import numpy as np
from mlx.utils import tree_flatten, tree_map, tree_unflatten

from ..metrics import summarize
from ..paths import ROOT, read_json, read_rows, sha256, write_json
from .coder_model import CONFIG, load_coder, prompt, score, save, restore
from .coder_assets import verify as verify_assets
from .data import DATA, REVIEW_ID, verify
from .final_cases import FINAL, rows as final_rows
from .coder_protocol import freeze

RUN=ROOT/"runs/som/research/coder"
TRAIN_CONFIG={"learning_rate":2e-5,"rank":8,"scale":16,"gradient_accumulation":8,
              "kl_weight":.5,"max_epochs":1,"max_seconds":7200,"seed":44,
              "train_per_family":12,"validation_per_family":3,"max_tokens":1536}


def subset(path,per_family):
    rows=read_rows(path)
    chosen=[]
    for family in sorted({r["family"] for r in rows}):
        chosen.extend([r for r in rows if r["family"]==family][:per_family])
    random.Random(44).shuffle(chosen)
    return chosen


def assess(model,tokenizer,rows,destination):
    records=[]
    for row in rows:
        r=score(model,tokenizer,row)
        records.append({**r,"id":row["id"],"task":row["task"],
                        "gold_index":next(i for i,c in enumerate(row["candidates"]) if c["id"]==row["gold_candidate_id"])})
    write_json(destination,records)
    return {d:summarize([r for r in records if r["task"]==d]) for d in ("python","frontend")}


def checkpoint(model,optimizer,run,state):
    import uuid
    dest=run/"checkpoints"/f"{state['samples_seen']:06d}-{uuid.uuid4().hex[:8]}"
    save(model,dest)
    mx.eval(optimizer.state)
    mx.savez(str(dest/"optimizer.npz"),**dict(tree_flatten(optimizer.state)))
    mx.savez(str(dest/"rng.npz"),state=mx.random.state[0])
    write_json(dest/"trainer.json",state)
    write_json(run/"latest.json",{"checkpoint":str(dest.relative_to(run))})
    return dest


def train(run=RUN,resume=False):
    verify()
    verify_assets()
    final_rows()  # Verify the frozen new holdout; do not evaluate it here.
    run=Path(run)
    run.mkdir(parents=True,exist_ok=True)
    freeze(run)
    if (run/"latest.json").exists() and not resume:
        raise ValueError("Coder run exists; use --resume or another directory.")
    rows=subset(DATA/"train.jsonl",TRAIN_CONFIG["train_per_family"])
    val=subset(DATA/"validation.jsonl",TRAIN_CONFIG["validation_per_family"])
    state={"config":TRAIN_CONFIG,"samples_seen":0,"updates":0,"elapsed_seconds":0.,"history":[],
           "data_manifest_sha256":sha256(DATA/"manifest.json"),"coder_source_sha256":sha256(ROOT/"som-research-coder.sources.lock.json"),
           "final_holdout_sha256":sha256(FINAL/"manifest.json")}
    if resume:
        saved=run/read_json(run/"latest.json")["checkpoint"]
        previous=read_json(saved/"trainer.json")
        for key in ("config","data_manifest_sha256","coder_source_sha256","final_holdout_sha256"):
            if state[key]!=previous[key]:
                raise ValueError(f"Coder resume input changed: {key}")
        if previous.get("status")=="completed":
            print(json.dumps({"status":"completed","unchanged":True}),flush=True)
            return
        state=previous
    model,tokenizer=load_coder()
    optimizer=optim.AdamW(learning_rate=TRAIN_CONFIG["learning_rate"],weight_decay=0)
    optimizer.init(model.trainable_parameters())
    write_json(run/"protocol.json",{"training":TRAIN_CONFIG,"train_ids":[r["id"] for r in rows],
               "validation_ids":[r["id"] for r in val],"final_holdout_sha256":state["final_holdout_sha256"],
               "selection":"Compare baseline and tuned validation NLL; never use final holdout to choose weights.",
               "limits":"One bounded low-rate run with a KL penalty toward the pretrained coder's candidate probabilities."})
    if resume:
        restore(model,saved)
        optimizer.state=tree_unflatten(list(mx.load(str(saved/"optimizer.npz")).items()))
        mx.random.state=[mx.load(str(saved/"rng.npz"))["state"]]
    else:
        save(model,run/"initial")
        base_metrics=assess(model,tokenizer,val,run/"baseline-validation.json")
        state["baseline_validation"]=base_metrics
        initial=checkpoint(model,optimizer,run,state)
        write_json(run/"selected.json",{"checkpoint":str(initial.relative_to(run)),"kind":"pretrained_baseline",
                   "validation_nll":float(np.mean([m["nll"] for m in base_metrics.values()]))})
        print(json.dumps({"baseline_validation":{d:m["accuracy"] for d,m in base_metrics.items()}}),flush=True)
    # Teacher probabilities are immutable and come only from the original pretrained model.
    teacher_file=run/"teacher-logits.json"
    if not teacher_file.exists():
        if resume and state["samples_seen"]:
            raise ValueError("Missing teacher cache for resumed coder training.")
        teacher=[]
        for i,row in enumerate(rows):
            teacher.append(score(model,tokenizer,row)["logits"])
            if (i+1)%64==0:
                print(json.dumps({"teacher_examples":i+1,"total":len(rows)}),flush=True)
        write_json(teacher_file,teacher)
    teacher=read_json(teacher_file)
    encoded=[prompt(row,tokenizer) for row in rows]

    def loss(current,tokens,labels,gold,prior):
        z=current(mx.array([tokens]),labels)[0]
        log_p=z-mx.logsumexp(z)
        teacher_logits=mx.array(prior)
        log_t=teacher_logits-mx.logsumexp(teacher_logits)
        t=mx.exp(log_t)
        kl=mx.sum(t*(log_t-log_p))
        return -log_p[gold]+TRAIN_CONFIG["kl_weight"]*kl
    grad_fn=nn.value_and_grad(model,loss)
    stopped={"value":False}
    def handler(signum,frame):
        stopped["value"]=True
    old={s:signal.signal(s,handler) for s in (signal.SIGINT,signal.SIGTERM)}
    started,previous_elapsed=time.monotonic(),state["elapsed_seconds"]
    status="completed"
    try:
        model.train()
        while state["samples_seen"]<len(rows):
            if stopped["value"]:
                status="interrupted"; break
            if previous_elapsed+time.monotonic()-started>=TRAIN_CONFIG["max_seconds"]:
                status="time_limit"; break
            count=min(8,len(rows)-state["samples_seen"])
            accumulated=None; losses=[]
            for offset in range(count):
                i=state["samples_seen"]+offset
                row=rows[i]; tokens,labels=encoded[i]
                gold=next(j for j,c in enumerate(row["candidates"]) if c["id"]==row["gold_candidate_id"])
                value,grads=grad_fn(model,tokens,labels,gold,teacher[i])
                accumulated=grads if accumulated is None else tree_map(lambda a,b:a+b,accumulated,grads)
                mx.eval(value,accumulated)
                losses.append(value.item())
            grads=tree_map(lambda x:x/count,accumulated)
            grads,norm=optim.clip_grad_norm(grads,1.)
            mx.eval(norm)
            if not np.isfinite([*losses,norm.item()]).all():
                raise RuntimeError("Non-finite coder loss or gradient.")
            optimizer.update(model,grads)
            mx.eval(model.parameters(),optimizer.state)
            state["samples_seen"]+=count; state["updates"]+=1
            state["elapsed_seconds"]=previous_elapsed+time.monotonic()-started
            rowlog={"samples":state["samples_seen"],"loss":float(np.mean(losses)),"elapsed_seconds":state["elapsed_seconds"],
                    "peak_mlx_gib":mx.get_peak_memory()/1024**3}
            state["history"].append(rowlog)
            if state["updates"]%4==0:
                print(json.dumps(rowlog),flush=True)
            if state["updates"]%8==0:
                checkpoint(model,optimizer,run,state)
            del grads,accumulated
            mx.clear_cache()
        state["status"]=status
        state["elapsed_seconds"]=previous_elapsed+time.monotonic()-started
        if status=="completed":
            model.eval()
            state["tuned_validation"]=assess(model,tokenizer,val,run/"tuned-validation.json")
        dest=checkpoint(model,optimizer,run,state)
        if status=="completed":
            nll=float(np.mean([m["nll"] for m in state["tuned_validation"].values()]))
            selected=read_json(run/"selected.json")
            if nll<selected["validation_nll"]:
                write_json(run/"selected.json",{"checkpoint":str(dest.relative_to(run)),"kind":"trained_qlora",
                           "validation_nll":nll})
        write_json(run/"training_summary.json",{**state,"peak_mlx_gib":mx.get_peak_memory()/1024**3,
                   "trainable_parameters":sum(v.size for _,v in tree_flatten(model.trainable_parameters()))})
        print(json.dumps({"status":status,"samples_seen":state["samples_seen"],"selected":read_json(run/"selected.json")}),flush=True)
    finally:
        for s,h in old.items():
            signal.signal(s,h)
