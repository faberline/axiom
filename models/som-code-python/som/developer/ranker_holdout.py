"""Forty unused HumanEvalPack problem IDs, selected without model predictions."""
import json
import random
import subprocess
import pyarrow.parquet as pq

from ..paths import ROOT,read_json,write_json,sha256
from .public_cases import DATA as OLD,build,make_rows,rows as old_rows
from .public_assets import verify as verify_assets
from .modern_assets import MODEL_PATH
from .ranker_input import check_request,prompt

DATA=ROOT/"data/som/research/ranker-final"
FILES=("som/developer/ranker_holdout.py","som/developer/public_cases.py",
       "som/developer/public_oracle.py","developer-runtime/public-oracle.mjs",
       "som/developer/ranker_input.py")


def prepare():
    from transformers import AutoTokenizer
    verify_assets(); old_rows()
    tokenizer=AutoTokenizer.from_pretrained(MODEL_PATH,local_files_only=True)
    datasets={lang:{int(r["task_id"].split("/")[-1]):r for r in pq.read_table(OLD/"source"/f"{lang}.parquet").to_pylist()} for lang in ("python","js")}
    excluded=set(read_json(OLD/"manifest.json")["problem_ids"])
    ids=sorted((set(datasets["python"])&set(datasets["js"]))-excluded)
    rng=random.Random(50); rng.shuffle(ids); chosen=[]; output=[]; cases=[]; skipped=[]
    for problem in ids:
        try:
            pair=[build(datasets[lang][problem],lang) for lang in ("python","js")]
            records=[r for case in pair for r in make_rows(case,rng)]
            for r in records:
                check_request(r,tokenizer)
                for c in r["candidates"]:
                    if c["id"]!="__review__":
                        prompt(r["state"],r["question"],c["text"],tokenizer)
        except (ValueError,SyntaxError,subprocess.TimeoutExpired) as e:
            skipped.append({"problem":problem,"reason":str(e)[:200]}); continue
        cases.extend(pair); output.extend(records); chosen.append(problem)
        print(json.dumps({"fresh_problems":len(chosen),"target":40}),flush=True)
        if len(chosen)==40:
            break
    if len(chosen)!=40:
        raise ValueError("Insufficient unused eligible problem IDs.")
    rng.shuffle(output)
    write_json(DATA/"cases.json",cases); write_json(DATA/"rows.json",output)
    write_json(DATA/"manifest.json",{"seed":50,"problem_ids":chosen,"problems":40,"rows":240,
        "language_versions":80,"skipped":skipped,"excluded_v3_ids":sorted(excluded),
        "hashes":{str(p.relative_to(ROOT)):sha256(p) for p in (DATA/"cases.json",DATA/"rows.json",OLD/"manifest.json",OLD/"source/sources.json")},
        "code_sha256":{p:sha256(ROOT/p) for p in FILES},
        "selection":"Seeded unused IDs, reference/buggy oracle validity and input length only; no model predictions.",
        "scope":"Fresh for v4 local evaluation. Forty correlated Python/JavaScript problem pairs, three candidate variants each; public pretraining exposure unknown. JavaScript function tests do not establish browser/React/CSS readiness."})
    print(json.dumps({"status":"passed","rows":len(rows())}),flush=True)


def rows():
    verify_assets(); old_rows(); m=read_json(DATA/"manifest.json")
    for key in ("hashes","code_sha256"):
        for p,digest in m[key].items():
            if sha256(ROOT/p)!=digest:
                raise ValueError("Fresh holdout provenance changed.")
    if set(m["problem_ids"])&set(m["excluded_v3_ids"]):
        raise ValueError("Previously tested problems entered fresh holdout.")
    return read_json(DATA/"rows.json")


if __name__=="__main__":
    prepare()
