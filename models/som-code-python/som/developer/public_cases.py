"""Frozen, test-only repair decisions from pinned HumanEvalPack programs."""
import ast
import copy
import json
import random
import re
import subprocess
import sys

import pyarrow.parquet as pq

from ..paths import ROOT, read_json, sha256, write_json
from .data import REVIEW_ID, REVIEW_TEXT, QUESTION
from .modern_assets import MODEL_PATH
from .modern_input import prompt
from .public_oracle import inspect_python

DATA=ROOT/"data/humanevalpack-v3"
SEED=4712


def inspect_js(source):
    if re.search(r"\b(?:eval|Function|require|process|fetch|import|XMLHttpRequest|constructor|prototype|globalThis|global)\b",source):
        raise ValueError("Unsupported fixture operation.")


def mutations(source,language):
    seen={source}
    if language=="python":
        tree=ast.parse(source)
        flips={ast.Add:ast.Sub,ast.Sub:ast.Add,ast.Mult:ast.Add,ast.Div:ast.Mult,
               ast.Lt:ast.GtE,ast.LtE:ast.Gt,ast.Gt:ast.LtE,ast.GtE:ast.Lt,ast.Eq:ast.NotEq,ast.NotEq:ast.Eq,
               ast.And:ast.Or,ast.Or:ast.And}
        for index,node in enumerate(ast.walk(tree)):
            clone=copy.deepcopy(tree); target=list(ast.walk(clone))[index]
            if isinstance(node,(ast.BinOp,ast.BoolOp)) and type(node.op) in flips:
                target.op=flips[type(node.op)]()
            elif isinstance(node,ast.Compare) and node.ops and type(node.ops[0]) in flips:
                target.ops[0]=flips[type(node.ops[0])]();
            elif isinstance(node,ast.Constant) and type(node.value) in (int,bool):
                target.value=(not node.value) if type(node.value) is bool else node.value+1
            else:
                continue
            value=ast.unparse(clone)
            if value not in seen:
                seen.add(value); yield value
    else:
        pattern=r"===|!==|<=|>=|&&|\|\||(?<![+=])\+(?![+=])|(?<![-=])-+(?![-=>])|(?<![=<>])<(?![=])|(?<![=<>])>(?![=])|\b(?:true|false|0|1)\b|\.(?:every|some|map|filter|sort|reverse|trim|toLowerCase|toUpperCase)\("
        flips={"===":"!==","!==":"===","<=":"<",">=":">","&&":"||","||":"&&","+":"-","-":"+","<":">",">":"<",
               "true":"false","false":"true","0":"1","1":"0",".every(":".some(",".some(":".every(",
               ".map(":".filter(",".filter(":".map(",".sort(":".reverse(",".reverse(":".sort(",
               ".trim(":".trimStart(",".toLowerCase(":".toUpperCase(",".toUpperCase(":".toLowerCase("}
        for match in re.finditer(pattern,source):
            if match.group() not in flips:
                continue
            value=source[:match.start()]+flips[match.group()]+source[match.end():]
            if value not in seen:
                seen.add(value); yield value


def execute(language,sources,tests):
    if language=="python":
        command=[sys.executable,"-m","som.developer.public_oracle"]
    else:
        command=["node",str(ROOT/"developer-runtime/public-oracle.mjs")]
    p=subprocess.run(command,input=json.dumps({"sources":sources,"tests":tests}),text=True,capture_output=True,
                     timeout=max(10,len(sources)*1.0+3),cwd=ROOT)
    if p.returncode:
        raise RuntimeError(f"Public fixture worker failed: {p.stderr[-600:]}")
    return json.loads(p.stdout)


def build(row,language):
    good=row["declaration"]+row["canonical_solution"]
    bad=row["declaration"]+row["buggy_solution"]
    tests=row["test_setup"]+"\n"+row["test"]
    inspect=inspect_python if language=="python" else inspect_js
    for text in (good,bad,tests):
        inspect(text)
    if language=="python" and not any(isinstance(n,ast.Assert) for n in ast.walk(ast.parse(tests))):
        raise ValueError("No Python test assertions.")
    first=execute(language,[good,bad],tests)
    if not (first[0]["passed"] and not first[1]["passed"] and first[1]["valid_syntax"]):
        raise ValueError("Reference/buggy pair is not executable with unique labels.")
    variants=[]
    for value in mutations(good,language):
        if value.strip()==bad.strip():
            continue
        inspect(value); variants.append(value)
        if len(variants)==24:
            break
    results=execute(language,variants,tests)
    for value,result in zip(variants,results):
        if result["valid_syntax"] and not result["passed"]:
            return {"id":row["task_id"],"task":"python" if language=="python" else "frontend",
                    "family":"humaneval_"+row["task_id"].split("/")[-1],"requirement":row["docstring"],
                    "sources":[good,bad,value],"tests":tests,"oracle":[first[0],first[1],result]}
    raise ValueError("No extra syntax-valid failing mutation.")


def make_rows(case,rng):
    output=[]
    for variant,indices in enumerate(((0,1),(0,1,2),(1,2))):
        candidates=[{"id":f"patch-{rng.randrange(10**12):012d}","text":case["sources"][i]} for i in indices]
        gold=REVIEW_ID if variant==2 else candidates[0]["id"]
        candidates.append({"id":REVIEW_ID,"text":REVIEW_TEXT}); rng.shuffle(candidates)
        output.append({"id":f"humanevalpack:{case['id']}:{variant}","task":case["task"],"family":case["family"],
            "state":f"Language: {case['task']}.\nRequirement: {case['requirement']}\nCurrent implementation:\n{case['sources'][1]}",
            "question":QUESTION,"candidates":candidates,"gold_candidate_id":gold,"missing_correct_patch":variant==2})
    return output


def prepare():
    from transformers import AutoTokenizer
    source=read_json(DATA/"source/sources.json")
    for name,record in source["files"].items():
        if sha256(DATA/"source"/name)!=record["sha256"]:
            raise ValueError("Public source changed.")
    tokenizer=AutoTokenizer.from_pretrained(MODEL_PATH,local_files_only=True)
    datasets={lang:{int(r["task_id"].split("/")[-1]):r for r in pq.read_table(DATA/"source"/f"{lang}.parquet").to_pylist()} for lang in ("python","js")}
    ids=sorted(set(datasets["python"]) & set(datasets["js"])); rng=random.Random(SEED); rng.shuffle(ids)
    accepted=[]; output=[]; skipped=[]; chosen=[]
    for problem in ids:
        try:
            pair=[build(datasets[lang][problem],lang) for lang in ("python","js")]
            rows=[r for case in pair for r in make_rows(case,rng)]
            for row in rows:
                prompt(row,tokenizer)
        except (ValueError,SyntaxError,subprocess.TimeoutExpired) as error:
            skipped.append({"problem":problem,"reason":str(error)[:200]}); continue
        accepted.extend(pair); output.extend(rows); chosen.append(problem)
        print(json.dumps({"public_problems":len(chosen),"target":40}),flush=True)
        if len(chosen)==40:
            break
    if len(chosen)!=40:
        raise ValueError("Not enough eligible public programs.")
    rng.shuffle(output)
    write_json(DATA/"cases.json",accepted)
    write_json(DATA/"rows.json",output)
    write_json(DATA/"manifest.json",{"seed":SEED,"problem_ids":chosen,"problems":40,"language_versions":80,"rows":240,
        "source_manifest_sha256":sha256(DATA/"source/sources.json"),"cases_sha256":sha256(DATA/"cases.json"),
        "rows_sha256":sha256(DATA/"rows.json"),"skipped":skipped,
        "code_sha256":{str(p.relative_to(ROOT)):sha256(p) for p in (
            ROOT/"som/developer/public_cases.py",ROOT/"som/developer/public_oracle.py",
            ROOT/"developer-runtime/public-oracle.mjs",ROOT/"som/developer/modern_input.py")},
        "scope":"Test only. Forty independent problem IDs, each in Python and JavaScript with three candidate variants. JavaScript functions do not establish DOM/React readiness.",
        "selection":"Seeded order, matching language IDs, executable reference and faulty candidates, token budget. No model predictions select cases.",
        "contamination":"Public benchmark may have appeared in base-model pretraining. No data from it is used for our training, validation selection, or calibration."})
    print(json.dumps({"public_cases":"verified","rows":len(output)}),flush=True)


def rows():
    manifest=read_json(DATA/"manifest.json")
    for field,name in (("rows_sha256","rows.json"),("cases_sha256","cases.json"),("source_manifest_sha256","source/sources.json")):
        if sha256(DATA/name)!=manifest[field]:
            raise ValueError("Public test data changed.")
    for path,digest in manifest["code_sha256"].items():
        if sha256(ROOT/path)!=digest:
            raise ValueError("Public test preparation code changed.")
    return read_json(DATA/"rows.json")


if __name__=="__main__":
    prepare()
