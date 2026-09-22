"""Remove answer-presence/count correlation; validate on separate families."""
import hashlib
import json
import random
from collections import Counter

from ..paths import ROOT, read_json, read_rows, sha256, write_json
from .data import DATA as OLD_DATA, REVIEW_ID, REVIEW_TEXT, verify as verify_original
from .modern_assets import MODEL_PATH, LOCK
from .modern_input import prompt

DATA=ROOT/"data/som/research/modern"
SEED=46


def rebalance(rows,cases,seed):
    rng=random.Random(seed)
    output=[]
    for domain in ("python","frontend"):
        for missing in (False,True):
            group=[r for r in rows if r["task"]==domain and r["missing_correct_patch"]==missing]
            rng.shuffle(group)
            for i,row in enumerate(group):
                case=cases[row["id"]]
                count=2+i%2  # Each label/domain group gets the same 2/3 patch mixture.
                codes=rng.sample(case["sources"][1:],count if missing else count-1)
                if not missing:
                    codes.append(case["sources"][0])
                rng.shuffle(codes)
                candidates=[{"id":f"patch-{rng.randrange(10**12):012d}","text":s} for s in codes]
                gold=REVIEW_ID if missing else next(c["id"] for c in candidates if c["text"]==case["sources"][0])
                candidates.append({"id":REVIEW_ID,"text":REVIEW_TEXT})
                rng.shuffle(candidates)
                output.append({**row,"candidates":candidates,"gold_candidate_id":gold})
    rng.shuffle(output)
    return output


def prepare():
    from transformers import AutoTokenizer
    verify_original()
    tokenizer=AutoTokenizer.from_pretrained(MODEL_PATH,local_files_only=True)
    cases={c["id"]:c for c in read_json(OLD_DATA/"oracles.json")}
    original_train=read_rows(OLD_DATA/"train.jsonl")
    rng=random.Random(SEED)
    train=[]
    for family in sorted({r["family"] for r in original_train}):
        # 7 missing and 17 present per family. All examples remain in their original split.
        for missing,n in ((True,7),(False,17)):
            group=[r for r in original_train if r["family"]==family and r["missing_correct_patch"]==missing]
            rng.shuffle(group); train.extend(group[:n])
    inputs={"train":train,"validation":read_rows(OLD_DATA/"challenge.jsonl"),
            "calibration":read_rows(OLD_DATA/"calibration.jsonl"),"seen":read_rows(OLD_DATA/"test.jsonl")}
    outputs={k:rebalance(v,cases,SEED+i) for i,(k,v) in enumerate(inputs.items())}
    DATA.mkdir(parents=True,exist_ok=True)
    counts={}; lengths=[]
    for split,rows in outputs.items():
        path=DATA/f"{split}.jsonl"
        path.write_text("".join(json.dumps(r)+"\n" for r in rows))
        lengths.extend(len(prompt(r,tokenizer)[0]) for r in rows)
        counts[split]={d:dict(Counter(f"{'missing' if r['missing_correct_patch'] else 'present'}:{len(r['candidates'])}" for r in rows if r["task"]==d)) for d in ("python","frontend")}
    write_json(DATA/"manifest.json",{"seed":SEED,"sha256":{k:sha256(DATA/f"{k}.jsonl") for k in outputs},
        "source_manifest_sha256":sha256(OLD_DATA/"manifest.json"),"model_lock_sha256":sha256(LOCK),
        "generator_sha256":sha256(ROOT/"som/developer/modern_data.py"),
        "prompt_sha256":sha256(ROOT/"som/developer/modern_input.py"),"counts":counts,"max_tokens":max(lengths),
        "validation_policy":"The eight previous challenge families are now development validation. They are never described as fresh v3 testing.",
        "count_policy":"Two or three supplied patches, balanced independently in each answer-presence/domain group.",
        "limits":"Training still contains 32 authored families. Public HumanEvalPack test data is never training or calibration data."})
    print(json.dumps(verify()),flush=True)


def verify():
    verify_original()
    m=read_json(DATA/"manifest.json")
    for name,path in (("source_manifest_sha256",OLD_DATA/"manifest.json"),("model_lock_sha256",LOCK),
                      ("generator_sha256",ROOT/"som/developer/modern_data.py"),
                      ("prompt_sha256",ROOT/"som/developer/modern_input.py")):
        if sha256(path)!=m[name]:
            raise ValueError(f"Modern data provenance changed: {name}")
    family_sets={}; ids=set(); states=set(); sizes={}
    for split,digest in m["sha256"].items():
        path=DATA/f"{split}.jsonl"
        if sha256(path)!=digest:
            raise ValueError(f"Modern data changed: {split}")
        rows=read_rows(path); sizes[split]=len(rows)
        family_sets[split]={r["family"] for r in rows}
        for r in rows:
            if r["id"] in ids or r["state"] in states:
                raise ValueError("Modern split overlap.")
            ids.add(r["id"]); states.add(r["state"])
        for domain in ("python","frontend"):
            for missing in (False,True):
                n=Counter(len(r["candidates"]) for r in rows if r["task"]==domain and r["missing_correct_patch"]==missing)
                if set(n)!={3,4} or abs(n[3]-n[4])>1:
                    raise ValueError("Candidate count exposes answer presence.")
    if family_sets["train"] & family_sets["validation"]:
        raise ValueError("Validation families overlap training.")
    return {"status":"passed","counts":sizes,"training_families":len(family_sets["train"]),
            "validation_families":len(family_sets["validation"]),"max_tokens":m["max_tokens"]}


if __name__=="__main__":
    prepare()
