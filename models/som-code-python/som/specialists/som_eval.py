"""Calibration-first and final-isolated evaluation for SOM v1."""
from __future__ import annotations

import hashlib
import json
import time
from pathlib import Path

from ..paths import read_json, resolve_checkpoint, sha256, write_json
from . import som_data
from .som_config import REVIEW_ID, run_path

# These names are populated only after ``require_ready`` succeeds. This keeps
# the guarded import path free of NumPy, SciPy, and MLX-related imports.
np = None
fit_temperature = None
decision = None
fail_closed_policy = None
family_metrics = None
gates = None
paired_family_bootstrap = None
wilson_upper = None
_rows = None
audit_or_raise = None
load_som_model = None
verify_pre_final = None


def _request(row):
    from .input import parse
    return parse({**row, "candidates": [candidate for candidate in row["candidates"] if candidate["id"] != REVIEW_ID]})


def _record(scorer, row):
    request = _request(row)
    started = time.perf_counter()
    scores = scorer.score(request)
    normal = time.perf_counter() - started
    reverse = _request({**row, "candidates": list(reversed(row["candidates"]))})
    started = time.perf_counter()
    reverse_scores = scorer.score(reverse)
    reverse_seconds = time.perf_counter() - started
    candidate_ids = [candidate.id for candidate in request.candidates] + [REVIEW_ID]
    reverse_ids = [candidate.id for candidate in reverse.candidates] + [REVIEW_ID]
    gold = len(request.candidates) if row["missing_correct_patch"] else next(i for i, candidate in enumerate(request.candidates) if candidate.id == row["gold_candidate_id"])
    return {"id": row["id"], "family": row["family"], "capability": row["capability"], "source_kind": row.get("source_kind", "unknown"),
            "missing": bool(row["missing_correct_patch"]), "candidate_ids": candidate_ids, "logits": list(scores) + [0.0],
            "reverse_candidate_ids": reverse_ids, "reverse_logits": list(reverse_scores) + [0.0], "gold_index": gold,
            "candidate_sources": [candidate.get("source", "unknown") for candidate in row["candidates"]] + ["review"],
            "candidate_text_lengths": [len(candidate.text) for candidate in request.candidates],
            "seconds": normal, "order_probe_seconds": reverse_seconds, "candidate_count": len(request.candidates)}


def _collect(model, tokenizer, rows):
    from .model import Scorer
    scorer = Scorer(model, tokenizer)
    return [_record(scorer, row) for row in rows]


def _threshold(records, temperature: float) -> dict:
    trials = []
    for threshold in (0.0, .50, .60, .70, .80, .85, .90, .95, .975, .99):
        choices = [decision(row, temperature, threshold) for row in records]
        accepted = [row for row in choices if row["accepted"]]
        trials.append({"threshold": threshold, "accepted": len(accepted), "coverage": len(accepted) / len(records) if records else 0.0,
                       "wilson_error_upper": wilson_upper(sum(not row["correct"] for row in accepted), len(accepted))})
    valid = [row for row in trials if row["accepted"] >= 40 and row["wilson_error_upper"] <= .10]
    return {"selected": max(valid, key=lambda row: (row["coverage"], row["threshold"])) if valid else
            {"threshold": None, "accepted": 0, "coverage": 0.0, "wilson_error_upper": 1.0}, "trials": trials}


def _latency(records):
    result = {}
    for count in (3, 7):
        values = [row["seconds"] for row in records if row["candidate_count"] == count]
        result[f"p95_{count}"] = float(np.percentile(values, 95)) if values else None
    return result


def _paraphrase_row(row):
    value = row.get("paraphrase") or row.get("paraphrase_question")
    if isinstance(value, dict):
        return {**row, **value}
    if isinstance(value, str) and value.strip():
        return {**row, "question": value}
    raise ValueError(f"Final family {row['family']} has no authored paraphrase.")


def _load_comparator(name: str, domain: str, checkpoint: Path):
    if name == "specialist":
        return load_som_model(checkpoint)
    if name == "mixed_control":
        return load_som_model(resolve_checkpoint(run_path("mixed")))
    from .model import load_model
    if name == "som_seed":
        return load_model(start_from_v4=True)
    if name == "untrained_scoring_module":
        return load_model()
    raise ValueError(f"Unknown comparator {name}.")


def _comparator_records(name: str, domain: str, checkpoint: Path, final_rows, temperature: float):
    try:
        model, tokenizer = _load_comparator(name, domain, checkpoint)
        records = _collect(model, tokenizer, final_rows)
        return {"available": True, "metrics": family_metrics(records, temperature, None), "records": records}
    except (FileNotFoundError, ValueError):
        return {"available": False}


def _simple_comparators(records):
    def result(selected):
        correct = [choice(row) == row["gold_index"] for row, choice in zip(records, selected)]
        present = [value for value, row in zip(correct, records) if not row["missing"]]
        return {"available": True, "accuracy": float(np.mean(correct)) if correct else 0.0,
                "present_accuracy": float(np.mean(present)) if present else 0.0}
    random = {"available": True, "accuracy": float(np.mean([1 / len(row["candidate_ids"]) for row in records])) if records else 0.0,
              "present_accuracy": float(np.mean([1 / (len(row["candidate_ids"]) - 1) for row in records if not row["missing"]])) if any(not row["missing"] for row in records) else 0.0}
    return {"random": random, "always_review": result([len(row["candidate_ids"]) - 1 for row in records]),
            "candidate_position_rule": {**result([0 for _ in records]), "rule": "first candidate"},
            "candidate_length_rule": {**result([int(np.argmax(row["candidate_text_lengths"])) for row in records]), "rule": "longest candidate text"}}


def _simple_rule_audit(comparisons: dict) -> dict:
    failures = [name for name, value in comparisons.items() if name in {"random", "always_review", "candidate_position_rule", "candidate_length_rule"}
                and value.get("available") and (value.get("accuracy", 0.0) >= .80 or value.get("present_accuracy", 0.0) >= .85)]
    return {"gate": "simple_rule_readiness", "actual": failures, "target": [], "passed": not failures}


def oracle_provenance() -> dict:
    """Bind evaluation to candidate-level fixed-fixture oracle evidence."""
    return {"oracle": "fixed_fixture", **som_data.executable_oracle_evidence()}


def evaluate(domain: str = "frontend", run=None) -> dict:
    if domain not in ("frontend", "python", "mixed"):
        raise ValueError("SOM evaluation domain must be frontend, python, or mixed.")
    # Direct Python callers get the same fail-closed boundary as the CLI.
    from .som_preflight import require_ready
    if domain == "frontend":
        require_ready()
    else:
        require_ready("frontend" if domain == "mixed" else domain)
    global np, fit_temperature, decision, fail_closed_policy, family_metrics, gates, paired_family_bootstrap, wilson_upper
    global _rows, audit_or_raise, load_som_model, verify_pre_final
    import numpy as np_module
    from ..metrics import fit_temperature as fit_temperature_function
    from .som_metrics import (decision as decision_function, fail_closed_policy as policy_function,
                               family_metrics as metrics_function, gates as gates_function,
                               paired_family_bootstrap as bootstrap_function, wilson_upper as wilson_function)
    from .som_train import (_rows as rows_function, audit_or_raise as audit_function,
                            load_som_model as load_function, verify_pre_final as verify_function)
    np = np_module
    fit_temperature = fit_temperature_function
    decision = decision_function
    fail_closed_policy = policy_function
    family_metrics = metrics_function
    gates = gates_function
    paired_family_bootstrap = bootstrap_function
    wilson_upper = wilson_function
    _rows = rows_function
    audit_or_raise = audit_function
    load_som_model = load_function
    verify_pre_final = verify_function
    corpus_domain = "frontend" if domain == "mixed" else domain
    audit = verify_pre_final(corpus_domain)
    oracle = oracle_provenance()
    target = Path(run or run_path(domain))
    training = read_json(target / "training_summary.json")
    if training.get("status") != "completed":
        raise ValueError("Complete SOM formal training before final evaluation.")
    checkpoint = resolve_checkpoint(target)
    # Calibration is the only data used for temperature and threshold selection.
    calibration_rows = _rows("calibration", corpus_domain)
    model, tokenizer = load_som_model(checkpoint)
    calibration = _collect(model, tokenizer, calibration_rows)
    fitted = fit_temperature(calibration)
    selected = _threshold(calibration, fitted["temperature"])
    selection = {"protocol": "som-v1", "checkpoint": str(checkpoint), "temperature": fitted["temperature"], "threshold": selected["selected"],
                 "calibration_ids_sha256": hashlib.sha256(json.dumps([row["id"] for row in calibration_rows]).encode()).hexdigest(),
                 "final_used_for_selection": False}
    frozen_selection = target / "calibration-selection.json"
    if frozen_selection.exists() and read_json(frozen_selection) != selection:
        raise ValueError("Calibration selection changed; final remains isolated.")
    write_json(frozen_selection, selection)
    # The final set is not read until all selection values have been frozen.
    audit = audit_or_raise(corpus_domain)
    final_rows = _rows("final", corpus_domain)
    final = _collect(model, tokenizer, final_rows)
    metric = family_metrics(final, fitted["temperature"], selected["selected"]["threshold"])
    del model
    model, tokenizer = load_som_model(checkpoint)
    reloaded = _collect(model, tokenizer, final_rows)
    reload_error = max((float(np.max(np.abs(np.asarray(left["logits"]) - np.asarray(right["logits"])))) for left, right in zip(final, reloaded)), default=0.0)
    paraphrase = _collect(model, tokenizer, [_paraphrase_row(row) for row in final_rows])
    paraphrase_consistency = float(np.mean([decision(left, fitted["temperature"])["choice"] == decision(right, fitted["temperature"])["choice"] for left, right in zip(final, paraphrase)])) if final else 0.0
    comparisons = _simple_comparators(final)
    for name in ("untrained_scoring_module", "som_seed", "mixed_control"):
        value = _comparator_records(name, domain, checkpoint, final_rows, fitted["temperature"])
        comparisons[name] = {key: item for key, item in value.items() if key != "records"}
    try:
        from ..evaluate import QwenBaseline
        from .baseline import qwen_greedy_metrics
        comparisons["original_qwen"] = qwen_greedy_metrics(final_rows, QwenBaseline(max_tokens=1536))
    except (FileNotFoundError, ValueError):
        comparisons["original_qwen"] = {"available": False}
    mixed = _comparator_records("mixed_control", domain, checkpoint, final_rows, fitted["temperature"])
    specialization = {"available": False, "non_inferior": False, "improvement_claimed": False}
    if domain in ("frontend", "python") and mixed.get("available"):
        bootstrap = paired_family_bootstrap(final, mixed["records"], fitted["temperature"])
        specialization = {"available": True, "accuracy_difference": bootstrap["point"], "bootstrap": bootstrap,
                          "non_inferior": bootstrap["point"] >= -.02,
                          "improvement_claimed": bootstrap["point"] >= .02 and bootstrap["interval_95"][0] > 0}
    evaluated_gates = gates(metric, reload_error=reload_error, paraphrase_consistency=paraphrase_consistency, latency=_latency(final))
    evaluated_gates.append(_simple_rule_audit(comparisons))
    if domain in ("frontend", "python"):
        evaluated_gates.append({"gate": "mixed_non_inferiority", "actual": specialization["non_inferior"], "target": True, "passed": specialization["non_inferior"]})
    else:
        # The mixed control exists only as a matched comparison. It never replaces the
        # frontend deployment policy even if its numerical gates happen to pass.
        evaluated_gates.append({"gate": "mixed_control_not_deployable", "actual": False, "target": True, "passed": False})
    adapter_sha = sha256(checkpoint / "adapter.safetensors")
    policy = fail_closed_policy(adapter_sha, domain, evaluated_gates, temperature=fitted["temperature"], threshold=selected["selected"]["threshold"],
                                validated_max_tokens=1536, validated_candidate_counts=[len(row["candidates"]) for row in final_rows])
    output = target / "evaluation-som-v1" / adapter_sha[:16]
    report = {"status": "ready" if policy["acceptance_passed"] else "not_ready", "protocol": "som-v1", "domain": domain, "audit": audit,
              "checkpoint": str(checkpoint), "training": training, "oracle_provenance": oracle, "calibration": {**fitted, "threshold": selected}, "metrics": metric,
              "comparisons": comparisons, "gates": evaluated_gates, "reload_max_error": reload_error,
              "paraphrase_consistency": paraphrase_consistency, "specialization": specialization, "policy": policy,
              "limitations": ["Candidate ranking only.", "Supplied code is never executed.", "A failed gate returns human_review."]}
    write_json(output / "report.json", report)
    write_json(target / "evaluation.json", {"protocol": "som-v1", "report": str((output / "report.json").relative_to(target)), "report_sha256": sha256(output / "report.json")})
    write_json(checkpoint / "specialist_policy.json", policy)
    return report
