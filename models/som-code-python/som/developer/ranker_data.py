"""Versioned local data for independent candidate verification."""
import json
import random
from collections import Counter

from ..paths import ROOT, read_json, read_rows, sha256, write_json
from .data import DATA as OLD, REVIEW_ID, REVIEW_TEXT, QUESTION, instantiate, check_cases
from .modern_data import DATA as MODERN, verify as verify_old
from .modern_assets import MODEL_PATH
from .mbpp_cases import DATA as MBPP, verify as verify_mbpp
from .ranker_fixtures import fixtures
from .ranker_input import check_request, prompt

DATA=ROOT/"data/som/research/ranker"
QUESTIONS=(QUESTION,"Select the candidate that meets all requirements. Use human review if none is valid.",
           "Which replacement satisfies the stated behavior in full?")
FILES=("som/developer/ranker_data.py","som/developer/ranker_fixtures.py",
       "som/developer/ranker_input.py","som/developer/mbpp_cases.py")


def base_row(case,variant,missing,rng):
    return {"id":case["id"]+f":v{variant}","task":case["task"],"family":case["family"],
            "source_case_id":case["id"],"missing_correct_patch":missing,
            "state":f"Language: {case['task']}.\nRequirement: {case['requirement']}\nCurrent implementation:\n{case['sources'][1]}",
            "question":QUESTION}


def populate(rows,cases,rng,train=False):
    result=[]
    for domain in ("python","frontend"):
        for missing in (False,True):
            group=[r for r in rows if r["task"]==domain and r["missing_correct_patch"]==missing]
            rng.shuffle(group)
            for i,row in enumerate(group):
                case=cases[row.get("source_case_id",row["id"])]; n=2+i%2
                codes=rng.sample(case["sources"][1:],n if missing else n-1)
                if not missing:
                    codes.append(case["sources"][0])
                candidates=[{"id":f"patch-{rng.randrange(10**12):012d}","text":s} for s in codes]
                gold=REVIEW_ID if missing else next(c["id"] for c in candidates if c["text"]==case["sources"][0])
                candidates.append({"id":REVIEW_ID,"text":REVIEW_TEXT}); rng.shuffle(candidates)
                result.append({**row,"question":rng.choice(QUESTIONS) if train else QUESTION,
                               "candidates":candidates,"gold_candidate_id":gold})
    rng.shuffle(result); return result


def prepare():
    from transformers import AutoTokenizer
    verify_old(); verify_mbpp()
    tokenizer=AutoTokenizer.from_pretrained(MODEL_PATH,local_files_only=True)
    rng=random.Random(48); cases={c["id"]:c for c in read_json(OLD/"oracles.json")}
    planned={k:[] for k in ("train","validation","calibration")}
    old_train=read_rows(OLD/"train.jsonl")
    for family in sorted({r["family"] for r in old_train}):
        for missing,n in ((True,10),(False,22)):
            group=[r for r in old_train if r["family"]==family and r["missing_correct_patch"]==missing]
            rng.shuffle(group); planned["train"].extend(group[:n])
    new_cases=[]
    for split,fixture in fixtures():
        count={"train":24,"validation":16,"calibration":20}[split]
        for i in range(count):
            case=instantiate(fixture,i,rng); new_cases.append(case); cases[case["id"]]=case
            planned[split].append(base_row(case,0,i<round(count*.3),rng))
    outcomes=check_cases(new_cases)
    mbpp=read_json(MBPP/"cases.json"); val=[c for c in mbpp if c["official_split"]=="validation"]
    rng.shuffle(val); val_ids={c["id"] for c in val[:len(val)//2]}
    mbpp_excluded=[]; mbpp_splits={k:[] for k in planned}
    for case in mbpp:
        split="train" if case["official_split"]=="train" else ("validation" if case["id"] in val_ids else "calibration")
        # Fit all possible 3-patch sets before adding any variant of this problem.
        import itertools
        try:
            for codes in itertools.combinations(case["sources"],3):
                probe=base_row(case,0,False,rng)
                probe["candidates"]=[{"id":str(i),"text":s} for i,s in enumerate(codes)]+[{"id":REVIEW_ID,"text":REVIEW_TEXT}]
                check_request(probe,tokenizer)
                for s in codes:
                    for q in QUESTIONS:
                        prompt(probe["state"],q,s,tokenizer)
        except ValueError as e:
            mbpp_excluded.append({"id":case["id"],"reason":str(e)}); continue
        cases[case["id"]]=case; mbpp_splits[split].append(case["id"])
        for variant in range(4):
            planned[split].append(base_row(case,variant,variant==3,rng))
    for domain in ("python","frontend"):
        pool=[r for r in read_rows(MODERN/"calibration.jsonl") if r["task"]==domain]
        rng.shuffle(pool); planned["calibration"].extend(pool[:80])
    output={s:populate(rows,cases,rng,train=s=="train") for s,rows in planned.items()}
    output["seen"]=read_rows(MODERN/"seen.jsonl")
    lengths=[]
    for split,rows in output.items():
        for row in rows:
            lengths.append(check_request(row,tokenizer)[1])
            for c in row["candidates"]:
                if c["id"]!=REVIEW_ID:
                    prompt(row["state"],row["question"],c["text"],tokenizer)
        DATA.mkdir(parents=True,exist_ok=True)
        (DATA/f"{split}.jsonl").write_text("".join(json.dumps(r)+"\n" for r in rows))
    write_json(DATA/"new-frontend-oracles.json",new_cases)
    write_json(DATA/"new-frontend-results.json",outcomes)
    write_json(DATA/"manifest.json",{
        "seed":48,"sha256":{s:sha256(DATA/f"{s}.jsonl") for s in output},
        "source_hashes":{str(p.relative_to(ROOT)):sha256(p) for p in (OLD/"manifest.json",MODERN/"manifest.json",MBPP/"manifest.json",ROOT/"som-research-modern.sources.lock.json")},
        "code_sha256":{p:sha256(ROOT/p) for p in FILES},
        "oracle_sha256":{p:sha256(DATA/p) for p in ("new-frontend-oracles.json","new-frontend-results.json")},
        "counts":{s:dict(Counter(r["task"] for r in rows)) for s,rows in output.items()},
        "mbpp_ids":mbpp_splits,"mbpp_excluded":mbpp_excluded,"max_tokens":max(lengths),
        "split_policy":"MBPP official train only for training; official validation problem IDs split between validation and calibration. No official MBPP test. New frontend families split 16/4/4. Prior authored calibration/seen instances remain in their original splits.",
        "count_policy":"2 or 3 supplied patches independently balanced within domain and answer-presence groups, plus review.",
        "limits":"Authored family instances are related. Four MBPP candidate variants share one problem. Tests establish finite-test labels, not universal correctness. Public HumanEvalPack is never training or calibration."})
    print(json.dumps(verify()),flush=True)


def verify():
    verify_old(); verify_mbpp(); m=read_json(DATA/"manifest.json")
    for key in ("source_hashes","code_sha256"):
        for p,digest in m[key].items():
            if sha256(ROOT/p)!=digest:
                raise ValueError(f"Ranker data provenance changed: {p}")
    for p,digest in m["oracle_sha256"].items():
        if sha256(DATA/p)!=digest:
            raise ValueError("Frontend oracle evidence changed.")
    ids=set(); prior_states=set(); families={}; sizes={}
    for split,digest in m["sha256"].items():
        if sha256(DATA/f"{split}.jsonl")!=digest:
            raise ValueError("Ranker rows changed.")
        rows=read_rows(DATA/f"{split}.jsonl"); sizes[split]=len(rows)
        states={r["state"] for r in rows}; current_ids={r["id"] for r in rows}
        if len(current_ids)!=len(rows) or current_ids&ids or states&prior_states:
            raise ValueError("Ranker cross-split overlap.")
        ids.update(current_ids); prior_states.update(states); families[split]={r["family"] for r in rows}
        for d in ("python","frontend"):
            for missing in (False,True):
                counts=Counter(len(r["candidates"]) for r in rows if r["task"]==d and r["missing_correct_patch"]==missing)
                if set(counts)!={3,4} or abs(counts[3]-counts[4])>1:
                    raise ValueError("Candidate count/answer presence imbalance.")
    if families["validation"]&(families["train"]|families["calibration"]):
        raise ValueError("Validation family leakage.")
    return {"status":"passed","counts":sizes,"domain_counts":m["counts"],"mbpp_counts":{s:len(v) for s,v in m["mbpp_ids"].items()},"max_tokens":m["max_tokens"]}


if __name__=="__main__":
    prepare()
