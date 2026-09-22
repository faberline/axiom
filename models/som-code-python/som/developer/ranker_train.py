"""One bounded independent-verifier run with exact frozen-prefix caches."""
import json
import signal
import time
import uuid
from pathlib import Path

import mlx.core as mx
import mlx.nn as nn
import mlx.optimizers as optim
import numpy as np
from mlx.utils import tree_flatten, tree_map, tree_unflatten

from ..metrics import summarize
from ..paths import ROOT, read_json, read_rows, sha256, write_json
from .modern_assets import LOCK, verify as verify_assets
from .ranker_data import DATA, verify as verify_data
from .ranker_loss import LOSS_CONFIG, features_for_row, loss
from .ranker_model import CONFIG, load_ranker, restore, save
from .ranker_holdout import DATA as PUBLIC, rows as public_rows
from .coder_protocol import CRITERIA
from .data import REVIEW_ID
from .ranker_cache import carried_seconds

RUN = ROOT / "runs/som/research/ranker"
TRAIN_CONFIG = {"learning_rate":1e-4,"rank":8,"scale":16,"gradient_accumulation":8,
                **LOSS_CONFIG,"max_epochs":1,"max_seconds":7200,"memory_gib":24,
                "seed":47,"max_tokens":1536,"candidate_scoring":"independent"}
BOUND_FILES = ("som/developer/ranker_train.py","som/developer/ranker_model.py",
               "som/developer/ranker_input.py","som/developer/ranker_data.py",
               "som/developer/ranker_loss.py","som/developer/policy.py",
               "som/developer/ranker_evaluate.py","som/metrics.py",
               "som/developer/coder_records.py","som/developer/modern_model.py",
               "som/developer/ranker_cache.py")


def protocol(run=RUN):
    return {"training":TRAIN_CONFIG,"model":CONFIG,"data_sha256":sha256(DATA/"manifest.json"),
            "preparation_carry_sha256":sha256(Path(run)/"preparation-carry.json") if (Path(run)/"preparation-carry.json").exists() else None,
            "public_sha256":sha256(PUBLIC/"manifest.json"),"source_sha256":sha256(LOCK),
            "code_sha256":{p:sha256(ROOT/p) for p in BOUND_FILES},
            "criteria":{k:list(v) for k,v in CRITERIA.items()},
            "selection":"Lowest domain-balanced mean validation NLL, among baseline, half epoch, and full epoch. Validation families are absent from training. Candidate order invariance is structural.",
            "test_policy":"HumanEvalPack is test only. No model scores selected its cases. Forty unused problem IDs have correlated language and candidate variants. Previous forty are regression only.",
            "claim_limit":"Function-level candidate repair pilot; no complete Python repository or DOM/React application acceptance."}


def freeze(run):
    value=protocol(run); path=run/"acceptance-protocol.json"
    if path.exists() and read_json(path)!=value:
        raise ValueError("Frozen ranker protocol changed.")
    write_json(path,value)
    return value


def verify_protocol(run):
    if read_json(Path(run)/"acceptance-protocol.json")!=protocol(run):
        raise ValueError("Ranker training, source, data, or acceptance protocol changed.")


def check_memory():
    peak=mx.get_peak_memory()/1024**3
    if peak>TRAIN_CONFIG["memory_gib"]:
        raise RuntimeError(f"MLX memory budget exceeded: {peak:.2f} GiB.")
    return peak


def checkpoint(model,optimizer,run,state):
    dest=run/"checkpoints"/f"{state['samples_seen']:06d}-{uuid.uuid4().hex[:8]}"
    save(model,dest)
    mx.eval(optimizer.state)
    mx.savez(str(dest/"optimizer.npz"),**dict(tree_flatten(optimizer.state)))
    mx.savez(str(dest/"rng.npz"),state=mx.random.state[0])
    write_json(dest/"trainer.json",state)
    write_json(run/"latest.json",{"checkpoint":str(dest.relative_to(run))})
    return dest


def make_features(model,tokenizer,rows,directory,stop):
    """Persist only outputs of the frozen prefix; teacher scores are pre-update."""
    directory.mkdir(parents=True,exist_ok=True)
    index_path=directory/"index.json"
    index=read_json(index_path) if index_path.exists() else {}
    for i,row in enumerate(rows):
        name=f"{i:06d}.safetensors"; dest=directory/name
        if name in index:
            if index[name]["id"]!=row["id"] or index[name]["sha256"]!=sha256(dest):
                raise ValueError("Frozen feature cache changed.")
            continue
        if stop():
            return False
        weights=features_for_row(model,tokenizer,row)
        mx.eval(weights)
        if not np.isfinite(weights["prior"].tolist()).all():
            raise RuntimeError("Non-finite teacher scores.")
        temp=directory/("temporary-"+name)
        mx.save_safetensors(str(temp),weights)
        temp.replace(dest)
        index[name]={"id":row["id"],"sha256":sha256(dest)}
        write_json(index_path,index)
        del weights
        mx.clear_cache(); check_memory()
        if (i+1)%32==0 or i+1==len(rows):
            print(json.dumps({"features":directory.name,"done":i+1,"total":len(rows)}),flush=True)
    if len(index)!=len(rows):
        raise ValueError("Unexpected feature cache length.")
    return True


def load_features(directory,index):
    return mx.load(str(directory/f"{index:06d}.safetensors"))


def assess(model,rows,directory,destination):
    model.eval(); records=[]
    for i,row in enumerate(rows):
        f=load_features(directory,i); n=int(f["count"].item())
        scores=mx.stack([model.score_features(f[f"hidden{j}"],f["labels"])[0] @ mx.array([1.,-1.]) for j in range(n)])
        logits=mx.concatenate([scores,mx.zeros((1,))]); mx.eval(logits)
        # Feature order is canonical candidate text/ID; final entry is review.
        records.append({"id":row["id"],"task":row["task"],"logits":logits.tolist(),
                        "gold_index":int(f["gold"].item()),"seconds":0})
    write_json(destination,records)
    metrics={d:summarize([r for r in records if r["task"]==d]) for d in ("python","frontend")}
    return {"domains":metrics,"mean_nll":float(np.mean([m["nll"] for m in metrics.values()])),
            "order_consistency":1.,"latency_note":"Cached validation; order invariance is structural, not a learned result. No inference latency measured here."}


def train(run=RUN,resume=False):
    verify_data(); verify_assets(); public_rows()
    run=Path(run); run.mkdir(parents=True,exist_ok=True)
    frozen=freeze(run)
    rows=read_rows(DATA/"train.jsonl")
    val=[{**r,"candidates":[c for c in r["candidates"] if c["id"]!=REVIEW_ID]
          +[c for c in r["candidates"] if c["id"]==REVIEW_ID]} for r in read_rows(DATA/"validation.jsonl")]
    state={"config":TRAIN_CONFIG,"protocol":frozen,"samples_seen":0,"updates":0,
           "elapsed_seconds":carried_seconds(run),"history":[],"status":"preparing"}
    if (run/"latest.json").exists():
        if not resume:
            raise ValueError("Ranker run exists; use --resume or another directory.")
        saved=run/read_json(run/"latest.json")["checkpoint"]
        state=read_json(saved/"trainer.json")
        if state["protocol"]!=frozen or state["config"]!=TRAIN_CONFIG:
            raise ValueError("Ranker resume inputs changed.")
        if state["status"]=="completed":
            print(json.dumps({"status":"completed","unchanged":True}),flush=True); return
    elif resume:
        raise ValueError("No ranker checkpoint to resume.")
    model,tokenizer=load_ranker()
    optimizer=optim.AdamW(learning_rate=TRAIN_CONFIG["learning_rate"],weight_decay=0)
    optimizer.init(model.trainable_parameters())
    if resume:
        restore(model,saved)
        optimizer.state=tree_unflatten(list(mx.load(str(saved/"optimizer.npz")).items()))
        mx.random.state=[mx.load(str(saved/"rng.npz"))["state"]]
    else:
        save(model,run/"initial")
        if (run/"preparation-carry.json").exists():
            carry=read_json(run/"preparation-carry.json")
            if sha256(run/"initial/adapter.safetensors")!=carry["initial_adapter_sha256"] or sha256(run/"features/train/index.json")!=carry["index_sha256"]:
                raise ValueError("Carried features do not match the fresh model or transferred index.")
        checkpoint(model,optimizer,run,state)
    stopped={"value":False}
    def handler(signum,frame):
        stopped["value"]=True
    handlers={s:signal.signal(s,handler) for s in (signal.SIGINT,signal.SIGTERM)}
    started=time.monotonic(); elapsed=state["elapsed_seconds"]
    def stop():
        state["elapsed_seconds"]=elapsed+time.monotonic()-started
        return stopped["value"] or state["elapsed_seconds"]>=TRAIN_CONFIG["max_seconds"]
    status="completed"
    train_dir=run/"features/train"; val_dir=run/"features/validation"
    try:
        if state["samples_seen"] and not state.get("features_complete"):
            raise ValueError("Cannot create teacher caches from trained weights.")
        if state.get("features_complete"):
            for directory,items in ((train_dir,rows),(val_dir,val)):
                index=read_json(directory/"index.json")
                if sha256(directory/"index.json")!=state["feature_hashes"][directory.name] or len(index)!=len(items):
                    raise ValueError("Feature cache index changed.")
                for i,row in enumerate(items):
                    entry=index[f"{i:06d}.safetensors"]
                    if entry["id"]!=row["id"] or entry["sha256"]!=sha256(directory/f"{i:06d}.safetensors"):
                        raise ValueError("Feature cache contents changed.")
        else:
            ready=make_features(model,tokenizer,rows,train_dir,stop) and make_features(model,tokenizer,val,val_dir,stop)
            if ready:
                state["features_complete"]=True
                state["feature_hashes"]={d.name:sha256(d/"index.json") for d in (train_dir,val_dir)}
        if not state.get("features_complete"):
            status="interrupted" if stopped["value"] else "time_limit"
        else:
            if "baseline_validation" not in state:
                metrics=assess(model,val,val_dir,run/"baseline-validation.json")
                state["baseline_validation"]=metrics; stop()
                initial=checkpoint(model,optimizer,run,state)
                write_json(run/"selected.json",{"checkpoint":str(initial.relative_to(run)),"kind":"pretrained_baseline","validation_nll":metrics["mean_nll"]})
                print(json.dumps({"baseline_validation":{d:m["accuracy"] for d,m in metrics["domains"].items()},"order_consistency":metrics["order_consistency"]}),flush=True)
            grad_fn=nn.value_and_grad(model,loss)
            while state["samples_seen"]<len(rows):
                if stop():
                    status="interrupted" if stopped["value"] else "time_limit"; break
                model.train(); count=min(8,len(rows)-state["samples_seen"]); accumulated=None; losses=[]
                for offset in range(count):
                    i=state["samples_seen"]+offset; row=rows[i]
                    gold=next(j for j,c in enumerate(row["candidates"]) if c["id"]==row["gold_candidate_id"])
                    value,grads=grad_fn(model,load_features(train_dir,i))
                    accumulated=grads if accumulated is None else tree_map(lambda a,b:a+b,accumulated,grads)
                    mx.eval(value,accumulated); losses.append(value.item())
                grads=tree_map(lambda x:x/count,accumulated)
                grads,norm=optim.clip_grad_norm(grads,1.)
                mx.eval(norm)
                if not np.isfinite([*losses,norm.item()]).all():
                    raise RuntimeError("Non-finite ranker loss or gradient.")
                optimizer.update(model,grads); mx.eval(model.parameters(),optimizer.state)
                state["samples_seen"]+=count; state["updates"]+=1; stop()
                entry={"samples":state["samples_seen"],"loss":float(np.mean(losses)),"elapsed_seconds":state["elapsed_seconds"],"peak_mlx_gib":check_memory()}
                state["history"].append(entry)
                if state["updates"]%4==0:
                    print(json.dumps(entry),flush=True)
                if (state["samples_seen"]>=len(rows)//2 and not state.get("half_assessed")) or state["samples_seen"]==len(rows):
                    state["half_assessed"]=True
                    metrics=assess(model,val,val_dir,run/f"validation-{state['samples_seen']:06d}.json")
                    state.setdefault("validation",{})[str(state["samples_seen"])]=metrics; stop()
                    dest=checkpoint(model,optimizer,run,state)
                    if metrics["mean_nll"]<read_json(run/"selected.json")["validation_nll"]:
                        write_json(run/"selected.json",{"checkpoint":str(dest.relative_to(run)),"kind":"trained_qlora","validation_nll":metrics["mean_nll"]})
                    print(json.dumps({"validation_samples":state["samples_seen"],"accuracy":{d:m["accuracy"] for d,m in metrics["domains"].items()},"order_consistency":metrics["order_consistency"]}),flush=True)
                elif state["updates"]%16==0:
                    checkpoint(model,optimizer,run,state)
                del grads,accumulated; mx.clear_cache()
        stop(); state["status"]=status
        checkpoint(model,optimizer,run,state)
        write_json(run/"training_summary.json",{**state,"peak_mlx_gib":check_memory(),
                   "trainable_parameters":sum(v.size for _,v in tree_flatten(model.trainable_parameters()))})
        print(json.dumps({"status":status,"samples_seen":state["samples_seen"],"elapsed_seconds":state["elapsed_seconds"]}),flush=True)
    finally:
        for s,h in handlers.items():
            signal.signal(s,h)


if __name__=="__main__":
    import argparse
    parser=argparse.ArgumentParser(); parser.add_argument("--run",type=Path,default=RUN); parser.add_argument("--resume",action="store_true")
    args=parser.parse_args(); train(args.run,args.resume)
