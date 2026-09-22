"""Calibration first; locked public testing and original-model comparison second."""
import gc
import json
from pathlib import Path

import mlx.core as mx
import numpy as np

from ..metrics import fit_temperature, summarize
from ..paths import ROOT, read_json, read_rows, resolve_checkpoint, sha256, write_json
from .coder_protocol import CRITERIA
from .coder_records import collect
from .data import REVIEW_ID
from .modern_assets import LOCK, verify as verify_assets
from .ranker_data import DATA, verify as verify_data
from .ranker_model import load_ranker, restore, Scorer
from .ranker_train import RUN, verify_protocol
from .policy import choose_threshold, decision, selective_metrics
from .ranker_holdout import DATA as PUBLIC, rows as public_rows
from .public_cases import rows as regression_rows


def runtime_order(rows):
    return [{**r,"candidates":[c for c in r["candidates"] if c["id"]!=REVIEW_ID]
             +[c for c in r["candidates"] if c["id"]==REVIEW_ID]} for r in rows]


def cluster_uncertainty(records,temperature,threshold):
    """Keep all candidate variants of one problem together during resampling."""
    families=sorted({r["family"] for r in records}); counts=[]
    for family in families:
        results=[decision(r,temperature) for r in records if r["family"]==family]
        selected=[r for r in results if threshold is not None and r["stable"]
                  and r["choice_id"]!=REVIEW_ID and r["confidence"]>=threshold]
        counts.append([sum(r["correct"] for r in results),len(results),
                       sum(not r["correct"] for r in selected),len(selected)])
    counts=np.asarray(counts); rng=np.random.default_rng(4701)
    samples=counts[rng.integers(0,len(families),size=(10000,len(families)))].sum(axis=1)
    error=np.divide(samples[:,2],samples[:,3],out=np.ones(10000),where=samples[:,3]>0)
    accuracy=samples[:,0]/samples[:,1]
    return {"independent_problem_ids":len(families),"bootstrap_repetitions":10000,
            "accuracy_interval_95":[float(v) for v in np.percentile(accuracy,[2.5,97.5])],
            "accepted_error_cluster_upper_95":float(np.percentile(error,95)),
            "note":"Resample problem IDs, retaining their correlated candidate variants. This is uncertainty on this selected corpus, not repository-wide performance."}


def evaluate(run=RUN):
    verify_data(); assets=verify_assets()
    run=Path(run); verify_protocol(run)
    training=read_json(run/"training_summary.json")
    if training["status"]!="completed":
        raise ValueError("Complete the bounded ranker run before final testing.")
    checkpoint=resolve_checkpoint(run); digest=sha256(checkpoint/"adapter.safetensors")
    output=run/"evaluation"/digest[:16]; output.mkdir(parents=True,exist_ok=True)
    provenance={"adapter_sha256":digest,"protocol_sha256":sha256(run/"acceptance-protocol.json"),
                "code":{p:sha256(ROOT/p) for p in ("som/developer/ranker_evaluate.py",
                    "som/developer/ranker_model.py","som/developer/ranker_input.py",
                    "som/developer/policy.py","som/metrics.py","som/developer/coder_records.py")}}
    if (output/"provenance.json").exists() and read_json(output/"provenance.json")!=provenance:
        raise ValueError("Ranker evaluation cache provenance changed.")
    write_json(output/"provenance.json",provenance)
    cal_rows=runtime_order(read_rows(DATA/"calibration.jsonl"))
    seen_rows=runtime_order(read_rows(DATA/"seen.jsonl"))
    final_rows=runtime_order(public_rows())
    model,tokenizer=load_ranker(checkpoint)
    scorer=Scorer(model,tokenizer)
    paired=scorer.paired
    cal=collect(cal_rows,paired,output/"calibration.json")
    policy={"adapter_sha256":digest,"acceptance_passed":False,"validated_max_tokens":0,
            "validated_candidate_counts":[3,4],"domains":{}}
    calibration={}
    for domain in ("python","frontend"):
        records=[r for r in cal if r["task"]==domain]
        fitted=fit_temperature(records); selected=choose_threshold(records,fitted["temperature"])
        calibration[domain]={**fitted,**selected}
        policy["domains"][domain]={"temperature":fitted["temperature"],"threshold":selected["selected"]["threshold"]}
    write_json(output/"calibration-policy-before-final.json",policy)
    seen=collect(seen_rows,paired,output/"seen.json")
    final=collect(final_rows,paired,output/"public-final.json")
    regression=collect(runtime_order(regression_rows()),paired,output/"previous-public-regression.json")
    questions=("Select the candidate that meets all requirements. Use human review if none is valid.",
               "Which replacement satisfies the stated behavior in full?")
    rewritten_rows=[{**r,"question":questions[i%2]} for d in ("python","frontend")
                    for i,r in enumerate([r for r in final_rows if r["task"]==d][:16])]
    rewrites=collect(rewritten_rows,lambda r:scorer.score(r,use_cache=False),output/"question-paraphrases.json")
    policy["validated_max_tokens"]=max(r["input_tokens"] for r in cal+seen+final)
    probe_rows=[r for d in ("python","frontend") for r in [x for x in final_rows if x["task"]==d][:16]]
    uncached=[]
    for row in probe_rows:
        normal=scorer.score(row,use_cache=False)
        reverse=scorer.score({**row,"candidates":list(reversed(row["candidates"]))},use_cache=False)
        difference=float(np.max(np.abs(np.asarray(normal["logits"])-np.asarray(reverse["logits"])[::-1])))
        uncached.append({"id":row["id"],"max_logit_error":difference,"seconds":normal["seconds"]+reverse["seconds"]})
    if max(r["max_logit_error"] for r in uncached)>1e-6:
        raise RuntimeError("Uncached candidate order invariance failed.")
    write_json(output/"uncached-order-probes.json",uncached)
    before=[scorer.score(r,use_cache=False)["logits"] for r in final_rows[:2]]
    del paired,scorer,model; gc.collect(); mx.clear_cache()
    model,tokenizer=load_ranker(checkpoint)
    scorer=Scorer(model,tokenizer)
    after=[scorer.score(r,use_cache=False)["logits"] for r in final_rows[:2]]
    reload_error=max(float(np.max(np.abs(np.asarray(a)-b))) for a,b in zip(before,after))
    if reload_error>1e-5:
        raise RuntimeError("Ranker save/reload changed scores.")
    restore(model,run/"initial")
    scorer=Scorer(model,tokenizer)
    base_seen=collect(seen_rows,lambda r:scorer.score(r,use_cache=False),output/"base-seen.json")
    base_final=collect(final_rows,lambda r:scorer.score(r,use_cache=False),output/"base-public-final.json")
    metrics={}; gates=[]
    for domain in ("python","frontend"):
        t=policy["domains"][domain]["temperature"]; threshold=policy["domains"][domain]["threshold"]
        sr=[r for r in seen if r["task"]==domain]; fr=[r for r in final if r["task"]==domain]
        sm=summarize(sr,t); fm=summarize(fr,t); selective=selective_metrics(fr,t,threshold)
        clusters=cluster_uncertainty(fr,t,threshold)
        upper=max(selective["error_wilson_upper_95"],clusters["accepted_error_cluster_upper_95"])
        rewritten=[r for r in rewrites if r["task"]==domain]; indexed={r["id"]:r for r in fr}
        agreement=float(np.mean([np.argmax(r["logits"])==np.argmax(indexed[r["id"]]["logits"]) for r in rewritten]))
        metrics[domain]={"seen":sm,"public_final":fm,"seen_raw":summarize(sr),"public_final_raw":summarize(fr),
            "previous_public_regression":summarize([r for r in regression if r["task"]==domain],t),
            "base_seen":summarize([r for r in base_seen if r["task"]==domain]),
            "base_public_final":summarize([r for r in base_final if r["task"]==domain]),
            "selective_public_final":selective,"selective_seen":selective_metrics(sr,t,threshold),
            "cluster_uncertainty":clusters,"accepted_error_gate_upper":upper,
            "question_paraphrase":{"n":len(rewritten),"choice_agreement":agreement,"metrics":summarize(rewritten,t)},
            "public_problems":{f:summarize([r for r in fr if r["family"]==f],t) for f in sorted({r["family"] for r in fr})},
            "paired_latency_ms":{f"p{q}":float(np.percentile([r["paired_seconds"]*1000 for r in sr+fr],q)) for q in (50,95)}}
        values={"seen_accuracy":sm["accuracy"],"fresh_family_accuracy":fm["accuracy"],
                "accepted_fresh_error_upper":upper,"accepted_fresh_count":selective["accepted"],
                "accepted_fresh_coverage":selective["coverage"],"fresh_order_consistency":selective["order_consistency"],
                "fresh_missing_fix_review_recall":selective["missing_fix_review_recall"]}
        for name,value in values.items():
            target,mode=CRITERIA[name]
            gates.append({"domain":domain,"gate":name,"actual":value,"target":target,"mode":mode,
                          "passed":value>=target if mode=="min" else value<=target})
    policy["acceptance_passed"]=all(g["passed"] for g in gates)
    write_json(checkpoint/"developer_policy.json",policy)
    report={"status":"bounded_pilot_passed" if policy["acceptance_passed"] else "not_ready",
            "checkpoint":str(checkpoint),"adapter_sha256":digest,"source":read_json(LOCK),
            "training":training,"selection":read_json(run/"selected.json"),"calibration":calibration,
            "verified_assets":assets,"policy":policy,"metrics":metrics,"gates":gates,
            "reload_max_logit_error":reload_error,"uncached_order_probes":uncached,
            "order_note":"Each candidate is scored independently. Reversal in paired timing is a cache lookup, not another GPU pass. All 32 additional order probes used uncached full inference.","public_manifest":read_json(PUBLIC/"manifest.json"),
            "evaluation_peak_mlx_gib":mx.get_peak_memory()/1024**3,
            "limitations":["Candidate repair selection, not free-form code generation.",
                "Forty public problem IDs, each with correlated Python/JavaScript and candidate variants; not 240 independent bugs.",
                "Public benchmark exposure during base-model pretraining is unknown.",
                "The forty v3 public IDs are regression only. Forty unused IDs are the fresh v4 test. No public scores select training checkpoints.",
                "JavaScript function tests do not validate DOM, CSS, React, browser workflows, or full repositories.",
                "A failed global gate requires human review on every API response. Offline selective counts do not mean API approval.",
                "The larger of the case Wilson upper bound and problem-cluster bootstrap upper bound controls the error gate."]}
    write_json(output/"report.json",report); write_json(run/"evaluation.json",{"report":str((output/"report.json").relative_to(run))})
    from .ranker_report import write_report
    write_report(report,run/"REPORT.md")
    print(json.dumps({"status":report["status"],"gates_passed":sum(g["passed"] for g in gates),"gates_total":len(gates),
                     "public_accuracy":{d:m["public_final"]["accuracy"] for d,m in metrics.items()}}),flush=True)
    return report


if __name__=="__main__":
    import argparse
    parser=argparse.ArgumentParser(); parser.add_argument("--run",type=Path,default=RUN)
    evaluate(parser.parse_args().run)
