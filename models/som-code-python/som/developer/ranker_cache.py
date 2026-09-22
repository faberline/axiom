"""Reuse only unchanged pre-update features, preserving the original time budget."""
import shutil
from pathlib import Path

from ..paths import ROOT,read_json,read_rows,sha256,write_json

FEATURE_CODE=("som/developer/ranker_model.py","som/developer/ranker_input.py",
              "som/developer/ranker_loss.py","som/developer/modern_model.py")


def transfer(source,destination):
    source=Path(source); destination=Path(destination)
    previous=source/read_json(source/"latest.json")["checkpoint"]
    state=read_json(previous/"trainer.json"); frozen=read_json(source/"acceptance-protocol.json")
    if state["samples_seen"]!=0 or state["updates"]!=0 or state["status"]!="interrupted":
        raise ValueError("Only gracefully interrupted, pre-update feature caches can transfer.")
    for name in FEATURE_CODE:
        if sha256(ROOT/name)!=frozen["code_sha256"][name]:
            raise ValueError("Feature model or prompt code differs.")
    if sha256(ROOT/"som-research-modern.sources.lock.json")!=frozen["source_sha256"]:
        raise ValueError("Feature base model source differs.")
    old_rows=read_rows(source/"inputs/train.jsonl")
    if sha256(source/"inputs/train.jsonl")!=read_json(source/"inputs/manifest.json")["sha256"]["train"]:
        raise ValueError("Archived source rows changed.")
    new_rows=read_rows(ROOT/"data/som/research/ranker/train.jsonl")
    new_indices={r["id"]:i for i,r in enumerate(new_rows)}
    old_directory=source/"features/train"; index=read_json(old_directory/"index.json")
    target=destination/"features/train"
    if target.exists():
        raise ValueError("Destination feature cache already exists.")
    target.mkdir(parents=True); transferred={}; reused=[]
    for name,entry in index.items():
        old_index=int(Path(name).stem); row=old_rows[old_index]
        if row["id"]!=entry["id"] or sha256(old_directory/name)!=entry["sha256"]:
            raise ValueError("Archived feature file changed.")
        i=new_indices.get(row["id"])
        if i is None or new_rows[i]!=row:
            continue
        filename=f"{i:06d}.safetensors"
        shutil.copy2(old_directory/name,target/filename)
        transferred[filename]=entry; reused.append(row["id"])
    write_json(target/"index.json",transferred)
    record={"source_run":str(source.resolve()),"source_protocol_sha256":sha256(source/"acceptance-protocol.json"),
        "source_trainer":str((previous/"trainer.json").resolve()),"source_trainer_sha256":sha256(previous/"trainer.json"),
        "initial_adapter_sha256":sha256(source/"initial/adapter.safetensors"),
        "elapsed_seconds":state["elapsed_seconds"],"transferred":len(transferred),"row_ids":reused,
        "index_sha256":sha256(target/"index.json"),"feature_code":{p:sha256(ROOT/p) for p in FEATURE_CODE},
        "reason":"Corrected MBPP formatting before the first optimizer update. Only exactly identical rows reused. All prior preparation time remains charged to the same two-hour budget."}
    write_json(destination/"preparation-carry.json",record)
    return {"transferred":len(transferred),"elapsed_seconds":state["elapsed_seconds"]}


def carried_seconds(run):
    run=Path(run); path=run/"preparation-carry.json"
    if not path.exists():
        return 0.
    record=read_json(path); previous=Path(record["source_trainer"])
    if sha256(previous)!=record["source_trainer_sha256"]:
        raise ValueError("Carried preparation evidence changed.")
    state=read_json(previous)
    if state["samples_seen"] or state["updates"] or state["elapsed_seconds"]!=record["elapsed_seconds"]:
        raise ValueError("Invalid carried preparation time.")
    for p,digest in record["feature_code"].items():
        if sha256(ROOT/p)!=digest:
            raise ValueError("Carried feature code changed.")
    if not 0<=record["elapsed_seconds"]<7200:
        raise ValueError("Carried preparation exhausted the time budget.")
    return record["elapsed_seconds"]


if __name__=="__main__":
    import argparse,json
    parser=argparse.ArgumentParser(); parser.add_argument("source",type=Path); parser.add_argument("destination",type=Path)
    args=parser.parse_args(); print(json.dumps(transfer(args.source,args.destination)))
