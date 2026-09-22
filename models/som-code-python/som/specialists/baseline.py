"""CPU-only conversion helpers for the original-Qwen comparison."""
import copy

import numpy as np

from .som_config import REVIEW_ID, REVIEW_TEXT

COMPARISON_KEYS=("specialist","random","simple_rule","v4","untrained_scoring_module","original_qwen_greedy","mixed")

def qwen_baseline_row(row):
    value=copy.deepcopy(row)
    value["candidates"].append({"id":REVIEW_ID,"text":REVIEW_TEXT})
    value["gold_candidate_id"]=REVIEW_ID if row["missing_correct_patch"] else row["gold_candidate_id"]
    return value

def qwen_greedy_metrics(rows, baseline):
    results=[]
    for row in rows:
        converted=qwen_baseline_row(row); value=baseline(converted); gold=next(i for i,item in enumerate(converted["candidates"]) if item["id"]==converted["gold_candidate_id"])
        greedy=value["greedy_index"]; results.append({"id":row["id"],"greedy_index":greedy,"gold_index":gold,"valid":greedy>=0,"correct":greedy==gold,**value})
    return {"available":True,"n":len(results),"greedy_accuracy":float(np.mean([item["correct"] for item in results])) if results else 0.,
            "invalid_outputs":sum(not item["valid"] for item in results),"invalid_output_rate":float(np.mean([not item["valid"] for item in results])) if results else 0.,"records":results}
