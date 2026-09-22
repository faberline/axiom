"""Fit review thresholds using calibration only; report uncertainty explicitly."""
import math

import numpy as np

from ..metrics import probabilities
from .data import REVIEW_ID


def wilson_upper(errors, n, z=1.959963984540054):
    if n == 0:
        return 1.0
    p = errors/n
    return (p + z*z/(2*n) + z*math.sqrt(p*(1-p)/n + z*z/(4*n*n))) / (1+z*z/n)


def decision(record, temperature):
    ids = record["candidate_ids"]
    p = probabilities(record["logits"], temperature)
    rp = probabilities(record["reverse_logits"], temperature)
    index = int(p.argmax())
    choice = ids[index]
    reverse_choice = record["reverse_candidate_ids"][int(rp.argmax())]
    reverse_index = record["reverse_candidate_ids"].index(choice)
    return {"choice_id": choice, "confidence": min(float(p[index]), float(rp[reverse_index])),
            "stable": choice == reverse_choice, "correct": index == record["gold_index"],
            "gold_is_review": ids[record["gold_index"]] == REVIEW_ID}


def selective_metrics(records, temperature, threshold):
    results = [decision(r, temperature) for r in records]
    accepted = [r for r in results if threshold is not None and r["stable"]
                and r["choice_id"] != REVIEW_ID and r["confidence"] >= threshold]
    errors = sum(not r["correct"] for r in accepted)
    absent = [r for r in results if r["gold_is_review"]]
    return {"n": len(records), "threshold": threshold, "accepted": len(accepted),
            "coverage": len(accepted)/len(records), "errors": errors,
            "error_rate": errors/len(accepted) if accepted else None,
            "error_wilson_upper_95": wilson_upper(errors, len(accepted)),
            "order_consistency": float(np.mean([r["stable"] for r in results])),
            "missing_fix_n": len(absent),
            "missing_fix_review_recall": float(np.mean([r["choice_id"] == REVIEW_ID for r in absent])) if absent else None,
            "forced_code_on_missing_fix": sum(r["choice_id"] != REVIEW_ID for r in absent)}


def choose_threshold(records, temperature):
    trials = [selective_metrics(records, temperature, t) for t in (0,.5,.6,.7,.8,.85,.9,.95,.975,.99)]
    eligible = [r for r in trials if r["accepted"] >= 50 and r["error_wilson_upper_95"] <= .10]
    selected = max(eligible, key=lambda r:(r["accepted"], r["threshold"])) if eligible else selective_metrics(records, temperature, None)
    return {"selected": selected, "calibration_trials": trials,
            "method": "Maximize coverage with at least 50 accepted calibration cases and a 95% Wilson error upper bound <= 10%. Break coverage ties using the highest threshold."}
