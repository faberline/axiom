import gc
import json
from pathlib import Path

import mlx.core as mx
import numpy as np

from .coder_records import collect
from .coder_report import write_report
from ..metrics import fit_temperature, summarize
from ..paths import ROOT, read_json, read_rows, resolve_checkpoint, sha256, write_json
from .coder_model import load_coder, paired_score, restore, score
from .coder_assets import verify as verify_assets
from .coder_train import RUN
from .data import DATA, REVIEW_ID, verify
from .evaluate import runtime_order
from .external import rows as public_rows
from .final_cases import FINAL, rows as fresh_rows
from .policy import choose_threshold, selective_metrics
from .coder_protocol import CRITERIA, verify as verify_protocol


def evaluate(run=RUN):
    verify()
    assets=verify_assets()
    run=Path(run)
    verify_protocol(run)
    training=read_json(run/"training_summary.json")
    if training["status"]!="completed":
        raise ValueError("Complete coder training before final evaluation.")
    if training["final_holdout_sha256"]!=sha256(FINAL/"manifest.json") or training["data_manifest_sha256"]!=sha256(DATA/"manifest.json"):
        raise ValueError("Coder evaluation data differs from the frozen training protocol.")
    checkpoint=resolve_checkpoint(run)
    digest=sha256(checkpoint/"adapter.safetensors")
    output=run/"evaluation"/digest[:16]
    output.mkdir(parents=True,exist_ok=True)
    provenance={"adapter_sha256":digest,"source_lock_sha256":sha256(ROOT/"som-research-coder.sources.lock.json"),
        "data_sha256":sha256(DATA/"manifest.json"),"final_sha256":sha256(FINAL/"manifest.json"),
        "scoring_code":{name:sha256(ROOT/name) for name in (
            "som/developer/coder_model.py","som/schema.py","som/developer/policy.py",
            "som/metrics.py","som/developer/coder_records.py")}}
    if (output/"provenance.json").exists() and read_json(output/"provenance.json")!=provenance:
        raise ValueError("Cached coder evaluation has different scoring code or inputs.")
    write_json(output/"provenance.json",provenance)
    cal_rows=runtime_order(read_rows(DATA/"calibration.jsonl"))
    seen_rows=runtime_order(read_rows(DATA/"test.jsonl"))
    final_rows=runtime_order(fresh_rows())
    probe_rows=runtime_order(public_rows())
    model,tokenizer=load_coder(checkpoint)
    scorer=lambda r:paired_score(model,tokenizer,r)
    cal=collect(cal_rows,scorer,output/"calibration.json")
    policy={"adapter_sha256":digest,"acceptance_passed":False,"validated_max_tokens":0,"domains":{},
            "validated_candidate_counts":sorted({len(r["candidates"]) for r in cal_rows+seen_rows+final_rows})}
    calibration={}
    for domain in ("python","frontend"):
        records=[r for r in cal if r["task"]==domain]
        fitted=fit_temperature(records)
        selected=choose_threshold(records,fitted["temperature"])
        calibration[domain]={**fitted,**selected}
        policy["domains"][domain]={"temperature":fitted["temperature"],"threshold":selected["selected"]["threshold"]}
    # The frozen calibration policy is written before any final predictions are read.
    write_json(output/"calibration-policy-before-final.json",policy)
    seen=collect(seen_rows,scorer,output/"seen.json")
    final=collect(final_rows,scorer,output/"fresh-final.json")
    questions=("Select the candidate that meets all requirements. Use human review if none is valid.",
               "Which replacement satisfies the stated behavior in full?")
    paraphrase_rows=[{**r,"question":questions[i%2]} for d in ("python","frontend")
                     for i,r in enumerate([r for r in final_rows if r["task"]==d][:16])]
    paraphrases=collect(paraphrase_rows,lambda r:score(model,tokenizer,r),output/"question-paraphrases.json")
    public=collect(probe_rows,lambda r:score(model,tokenizer,r),output/"public-diagnostic.json")
    policy["validated_max_tokens"]=max(r["input_tokens"] for r in cal+seen+final)
    logits_before=[score(model,tokenizer,row)["logits"] for row in final_rows[:2]]
    del model
    gc.collect(); mx.clear_cache()
    model,tokenizer=load_coder(checkpoint)
    logits_after=[score(model,tokenizer,row)["logits"] for row in final_rows[:2]]
    reload_error=max(float(np.max(np.abs(np.asarray(a)-b))) for a,b in zip(logits_before,logits_after))
    if reload_error>1e-5:
        raise RuntimeError("Coder reload changed scores.")
    # Report the exact pretrained starting point, using the same prompt and scorer.
    restore(model,run/"initial")
    base_seen=collect(seen_rows,lambda r:score(model,tokenizer,r),output/"base-seen.json")
    base_final=collect(final_rows,lambda r:score(model,tokenizer,r),output/"base-fresh-final.json")
    base_public=collect(probe_rows,lambda r:score(model,tokenizer,r),output/"base-public-diagnostic.json")
    metrics={}; gates=[]
    for domain in ("python","frontend"):
        temperature=policy["domains"][domain]["temperature"]
        threshold=policy["domains"][domain]["threshold"]
        sr=[r for r in seen if r["task"]==domain]
        fr=[r for r in final if r["task"]==domain]
        sm=summarize(sr,temperature); fm=summarize(fr,temperature)
        selective=selective_metrics(fr,temperature,threshold)
        rewritten=[r for r in paraphrases if r["task"]==domain]
        originals={r["id"]:r for r in fr}
        agreement=float(np.mean([int(np.argmax(r["logits"]))==int(np.argmax(originals[r["id"]]["logits"])) for r in rewritten]))
        metrics[domain]={"seen":sm,"fresh_final":fm,"seen_raw":summarize(sr),"fresh_final_raw":summarize(fr),
            "base_seen":summarize([r for r in base_seen if r["task"]==domain]),
            "base_fresh_final":summarize([r for r in base_final if r["task"]==domain]),
            "selective_fresh_final":selective,
            "selective_seen":selective_metrics(sr,temperature,threshold),
            "public_diagnostic":summarize([r for r in public if r["task"]==domain],temperature),
            "base_public_diagnostic":summarize([r for r in base_public if r["task"]==domain]),
            "question_paraphrase":{"n":len(rewritten),"choice_agreement":agreement,"metrics":summarize(rewritten,temperature)},
            "fresh_families":{family:summarize([r for r in fr if r["family"]==family],temperature)
                              for family in sorted({r["family"] for r in fr})},
            "paired_latency_ms":{f"p{q}":float(np.percentile([r["paired_seconds"]*1000 for r in sr+fr],q)) for q in (50,95)}}
        values={"seen_accuracy":sm["accuracy"],"fresh_family_accuracy":fm["accuracy"],
                "accepted_fresh_error_upper":selective["error_wilson_upper_95"],"accepted_fresh_count":selective["accepted"],
                "accepted_fresh_coverage":selective["coverage"],"fresh_order_consistency":selective["order_consistency"],
                "fresh_missing_fix_review_recall":selective["missing_fix_review_recall"]}
        for name,value in values.items():
            target,mode=CRITERIA[name]
            gates.append({"domain":domain,"gate":name,"actual":value,"target":target,"mode":mode,
                          "passed":value>=target if mode=="min" else value<=target})
    policy["acceptance_passed"]=all(g["passed"] for g in gates)
    write_json(checkpoint/"developer_policy.json",policy)
    report={"status":"bounded_pilot_passed" if policy["acceptance_passed"] else "not_ready",
        "checkpoint":str(checkpoint),"adapter_sha256":digest,"source":read_json(ROOT/"som-research-coder.sources.lock.json"),
        "training":training,"selection":read_json(run/"selected.json"),"calibration":calibration,"verified_assets":assets,
        "policy":policy,"metrics":metrics,"gates":gates,"reload_max_logit_error":reload_error,
        "fresh_manifest":read_json(FINAL/"manifest.json"),"evaluation_peak_mlx_gib":mx.get_peak_memory()/1024**3,
        "limitations":["This is a candidate selector, not free-form code generation.",
            "Fresh evaluation contains 12 authored families with 20 variants each; these are not 240 independent repository bugs.",
            "The six public functions were used as earlier diagnostics. Their scores are not a new untouched benchmark.",
            "Selected checkpoints use validation only. Calibration uses separate examples. Final cases never enter training.",
            "Human review is required for unsupported length, unstable order, low confidence, no valid patch, or failed acceptance.",
            "No production repository or complete frontend application has been accepted."]}
    write_json(output/"report.json",report)
    write_json(run/"evaluation.json",{"report":str((output/"report.json").relative_to(run))})
    write_report(report,run/"REPORT.md")
    print(json.dumps({"status":report["status"],"gates_passed":sum(g["passed"] for g in gates),"gates_total":len(gates),
                     "fresh_accuracy":{d:m["fresh_final"]["accuracy"] for d,m in metrics.items()}}),flush=True)
    return report

