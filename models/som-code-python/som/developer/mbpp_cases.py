"""Pinned MBPP training/validation programs with execution-checked mutations."""
import ast
import builtins
import copy
import json
import random
from pathlib import Path

import pyarrow.parquet as pq

from ..paths import ROOT,read_json,sha256,write_json
from .public_cases import mutations,execute
from .public_oracle import inspect_python

DATA=ROOT/"data/mbpp-v4"
LOCK=ROOT/"som-research-mbpp.sources.lock.json"


def fingerprint(source):
    tree=ast.parse(source); names={}; reserved=set(dir(builtins))
    def rename(name):
        if name in reserved:
            return name
        if name not in names:
            names[name]=f"symbol{len(names)}"
        return names[name]
    class Normalize(ast.NodeTransformer):
        def visit_FunctionDef(self,node):
            node.name=rename(node.name); return self.generic_visit(node)
        visit_AsyncFunctionDef=visit_FunctionDef
        def visit_ClassDef(self,node):
            node.name=rename(node.name); return self.generic_visit(node)
        def visit_Name(self,node):
            node.id=rename(node.id); return node
        def visit_arg(self,node):
            node.arg=rename(node.arg); return node
        def visit_Expr(self,node):
            return None if isinstance(node.value,ast.Constant) and isinstance(node.value.value,str) else self.generic_visit(node)
    return ast.dump(Normalize().visit(tree),include_attributes=False)


def variants(source):
    yield from mutations(source,"python")
    tree=ast.parse(source)
    for i,node in enumerate(ast.walk(tree)):
        if not isinstance(node,ast.Return) or node.value is None:
            continue
        for replacement in (ast.Constant(None),ast.List(elts=[],ctx=ast.Load()),ast.UnaryOp(op=ast.Not(),operand=copy.deepcopy(node.value))):
            clone=copy.deepcopy(tree); target=list(ast.walk(clone))[i]; target.value=replacement
            yield ast.unparse(ast.fix_missing_locations(clone))


def verify_sources():
    lock=read_json(LOCK)
    if lock!=read_json(DATA/"source/sources.json"):
        raise ValueError("MBPP source lock changed.")
    for name,record in lock["files"].items():
        if sha256(DATA/"source"/name)!=record["sha256"]:
            raise ValueError("MBPP source file changed.")
    return lock


def prepare():
    verify_sources()
    public=pq.read_table(ROOT/"data/humanevalpack-v3/source/python.parquet").to_pylist()
    excluded={fingerprint(r["declaration"]+r[key]) for r in public for key in ("canonical_solution","buggy_solution")}
    seen=set(); output=[]; skipped=[]
    for split in ("train","validation"):
        rows=pq.read_table(DATA/f"source/sanitized/{split}-00000-of-00001.parquet").to_pylist()
        random.Random(49).shuffle(rows)
        for row in rows:
            identifier=f"mbpp:{row['task_id']}"
            try:
                # Apply the same formatter to references and mutations. Otherwise
                # indentation and quote style can expose the positive label.
                good=ast.unparse(ast.parse(row["code"]))
                tests="\n".join([*row["test_imports"],*row["test_list"]])
                inspect_python(good); inspect_python(tests)
                signature=fingerprint(good)
                if signature in excluded or signature in seen:
                    raise ValueError("Duplicate normalized code, including HumanEvalPack exclusion.")
                first=execute("python",[good],tests)[0]
                if not first["passed"]:
                    raise ValueError("Reference failed its published tests.")
                choices=[]
                for candidate in variants(good):
                    if candidate not in choices and fingerprint(candidate)!=signature:
                        inspect_python(candidate); choices.append(candidate)
                    if len(choices)>=24:
                        break
                bad=[]; outcomes=[]
                for start in range(0,len(choices),4):
                    batch=choices[start:start+4]
                    results=execute("python",batch,tests)
                    for source,result in zip(batch,results):
                        if result["valid_syntax"] and not result["passed"]:
                            bad.append(source); outcomes.append(result)
                    if len(bad)>=3:
                        break
                if len(bad)<3:
                    raise ValueError("Fewer than three distinct failing mutations.")
                seen.add(signature)
                output.append({"id":identifier,"task":"python","family":identifier,"official_split":split,
                    "requirement":row["prompt"],"sources":[good,*bad[:3]],"checks":tests,"dom":False,
                    "oracle":[first,*outcomes[:3]],"source_task_id":row["task_id"]})
            except (ValueError,SyntaxError) as error:
                skipped.append({"id":identifier,"reason":str(error)[:200]})
            if (len(output)+len(skipped))%16==0:
                print(json.dumps({"mbpp_checked":len(output)+len(skipped),"accepted":len(output)}),flush=True)
    if sum(c["official_split"]=="train" for c in output)<50 or sum(c["official_split"]=="validation" for c in output)<16:
        raise ValueError("Insufficient verified MBPP programs.")
    write_json(DATA/"cases.json",output)
    write_json(DATA/"manifest.json",{"lock_sha256":sha256(LOCK),"cases_sha256":sha256(DATA/"cases.json"),
        "code_sha256":sha256(Path(__file__)),"counts":{s:sum(c["official_split"]==s for c in output) for s in ("train","validation")},
        "skipped":skipped,"deduplication":"Normalized ASTs checked against all 164 HumanEvalPack Python references and buggy versions. Public data never becomes training data.",
        "formatting":"All reference and mutated candidates use ast.unparse before execution checks, removing reference-only indentation/quote cues.",
        "labels":"Correctness here means passing the published finite tests; it is not a proof for every possible input."})
    print(json.dumps(verify()),flush=True)


def verify():
    verify_sources(); m=read_json(DATA/"manifest.json")
    if m["lock_sha256"]!=sha256(LOCK) or m["cases_sha256"]!=sha256(DATA/"cases.json") or m["code_sha256"]!=sha256(Path(__file__)):
        raise ValueError("MBPP prepared cases changed.")
    return {"status":"passed","counts":m["counts"]}


if __name__=="__main__":
    prepare()
