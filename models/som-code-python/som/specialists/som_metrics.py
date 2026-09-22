"""CPU-only reliability accounting for the isolated SOM v1 protocol."""
from __future__ import annotations

from collections import defaultdict
from math import sqrt
from typing import Mapping, Sequence

import numpy as np

from .som_config import GATES, REVIEW_ID

_Z = 1.959963984540054


def probabilities(logits: Sequence[float], temperature: float = 1.0) -> np.ndarray:
    if temperature <= 0:
        raise ValueError("Temperature must be positive.")
    values = np.asarray(logits, dtype=np.float64) / temperature
    values -= values.max()
    values = np.exp(values)
    return values / values.sum()


def choose(ids: Sequence[str], values: Sequence[float]) -> int:
    """Choose the reserved review option on an exact maximum tie."""
    if not ids or len(ids) != len(values):
        raise ValueError("Choice IDs and values must be nonempty and aligned.")
    maximum = max(values)
    tied = [i for i, value in enumerate(values) if value == maximum]
    return next((i for i in tied if ids[i] == REVIEW_ID), tied[0])


def decision(record: Mapping, temperature: float, threshold: float | None = None) -> dict:
    values = probabilities(record["logits"], temperature)
    index = choose(record["candidate_ids"], values.tolist())
    choice = record["candidate_ids"][index]
    reverse_values = probabilities(record["reverse_logits"], temperature)
    reverse_index = choose(record["reverse_candidate_ids"], reverse_values.tolist())
    reverse_choice = record["reverse_candidate_ids"][reverse_index]
    stable = choice == reverse_choice
    return {"choice": choice, "reverse_choice": reverse_choice, "correct": index == record["gold_index"],
            "stable": stable, "confidence": float(values[index]),
            "accepted": threshold is not None and choice != REVIEW_ID and stable and values[index] >= threshold}


def wilson_upper(errors: int, n: int) -> float:
    if not n:
        return 1.0
    if not 0 <= errors <= n:
        raise ValueError("Wilson error count is invalid.")
    rate = errors / n
    return float((rate + _Z * _Z / (2 * n) + _Z * sqrt(rate * (1 - rate) / n + _Z * _Z / (4 * n * n))) / (1 + _Z * _Z / n))


def _rate(rows, key):
    return float(np.mean([row[key] for row in rows])) if rows else 0.0


def smoke_metric_breakdown(*, loss: float, correct: int, rows: int,
                           present_correct: int, present_rows: int,
                           missing_review_correct: int, missing_rows: int) -> dict:
    """Return deterministic metrics for a bounded smoke pass.

    The smoke run has no candidate-order probe or calibration step.  These
    counters make its result useful while it is still running and avoid
    hiding a present/missing tradeoff behind one overall accuracy value.
    """
    if min(correct, present_correct, missing_review_correct, rows,
           present_rows, missing_rows) < 0:
        raise ValueError("Smoke metric counts cannot be negative.")
    if present_rows + missing_rows != rows:
        raise ValueError("Smoke present and missing counts must equal rows.")
    if correct > rows or present_correct > present_rows or missing_review_correct > missing_rows:
        raise ValueError("Smoke metric correct counts exceed their row counts.")
    if correct != present_correct + missing_review_correct:
        raise ValueError("Smoke overall correct count does not match present and missing counts.")
    return {
        "loss": float(loss),
        "accuracy": float(correct / rows) if rows else 0.0,
        "rows": int(rows),
        "correct": int(correct),
        "present_accuracy": float(present_correct / present_rows) if present_rows else 0.0,
        "present_rows": int(present_rows),
        "present_correct": int(present_correct),
        "missing_review_recall": float(missing_review_correct / missing_rows) if missing_rows else 0.0,
        "missing_rows": int(missing_rows),
        "missing_review_correct": int(missing_review_correct),
    }


def family_metrics(records: Sequence[Mapping], temperature: float, threshold: float | None) -> dict:
    """Calculate case and family measures without flattening family weights."""
    outcomes = [(row, decision(row, temperature, threshold)) for row in records]
    present = [value for row, value in outcomes if not row["missing"]]
    missing = [value for row, value in outcomes if row["missing"]]
    accepted = [(row, value) for row, value in outcomes if value["accepted"]]
    groups = defaultdict(list)
    for row, value in outcomes:
        groups[row["family"]].append((row, value))
    per_family = {}
    for family, rows in sorted(groups.items()):
        family_present = [value for row, value in rows if not row["missing"]]
        family_missing = [value for row, value in rows if row["missing"]]
        per_family[family] = {"n": len(rows), "accuracy": _rate([value for _, value in rows], "correct"),
                              "present": len(family_present), "missing": len(family_missing),
                              "present_accuracy": _rate(family_present, "correct") if family_present else None,
                              "missing_review_recall": _rate([{**value, "review": value["choice"] == REVIEW_ID and value["stable"]} for value in family_missing], "review") if family_missing else None,
                              "accepted": sum(value["accepted"] for _, value in rows)}
    accepted_groups = defaultdict(list)
    for row, value in accepted:
        accepted_groups[row["family"]].append(value)
    # A family is an accepted success only if every accepted decision in it is correct.
    accepted_family_errors = sum(not all(value["correct"] for value in values) for values in accepted_groups.values())
    by_field = {}
    for field in ("capability", "source_kind"):
        groups_for_field = defaultdict(list)
        for row, value in outcomes:
            groups_for_field[row.get(field, "unknown")].append(value)
        by_field[field] = {name: {"n": len(values), "accuracy": _rate(values, "correct")}
                           for name, values in sorted(groups_for_field.items())}
    candidate_sources = defaultdict(int)
    for row, _ in outcomes:
        for source in row.get("candidate_sources", ("unknown",)):
            candidate_sources[source] += 1
    by_field["candidate_source"] = {name: {"n": count} for name, count in sorted(candidate_sources.items())}
    return {"overall": {"n": len(records), "accuracy": _rate([value for _, value in outcomes], "correct")},
            "present_accuracy": _rate(present, "correct"),
            "missing_review_recall": _rate([{**value, "review": value["choice"] == REVIEW_ID and value["stable"]} for value in missing], "review"),
            "order_consistency": _rate([value for _, value in outcomes], "stable"),
            "coverage": len(accepted) / len(records) if records else 0.0,
            "accepted": len(accepted), "accepted_wilson_error_upper": wilson_upper(sum(not value["correct"] for _, value in accepted), len(accepted)),
            "family_macro_accuracy": float(np.mean([value["accuracy"] for value in per_family.values()])) if per_family else 0.0,
            "family_macro_present_accuracy": float(np.mean([value["present_accuracy"] for value in per_family.values() if value["present_accuracy"] is not None])) if any(value["present_accuracy"] is not None for value in per_family.values()) else 0.0,
            "family_macro_missing_review_recall": float(np.mean([value["missing_review_recall"] for value in per_family.values() if value["missing_review_recall"] is not None])) if any(value["missing_review_recall"] is not None for value in per_family.values()) else 0.0,
            "family": per_family, "accepted_families": len(accepted_groups),
            "accepted_family_wilson_error_upper": wilson_upper(accepted_family_errors, len(accepted_groups)),
            "breakdowns": by_field, "decisions": [value for _, value in outcomes]}


def paired_family_bootstrap(left: Sequence[Mapping], right: Sequence[Mapping], temperature: float, *, repetitions: int = 10_000, seed: int = 4701) -> dict:
    """Equal-family paired bootstrap; IDs and families must match exactly."""
    right_by_id = {row["id"]: row for row in right}
    if len(right_by_id) != len(right) or {row["id"] for row in left} != set(right_by_id):
        raise ValueError("Compared records have different IDs.")
    grouped = defaultdict(list)
    for row in left:
        other = right_by_id[row["id"]]
        if row["family"] != other["family"]:
            raise ValueError("Compared records have different families.")
        grouped[row["family"]].append(float(decision(row, temperature)["correct"] - decision(other, temperature)["correct"]))
    values = np.asarray([np.mean(grouped[key]) for key in sorted(grouped)], dtype=float)
    if not len(values):
        return {"families": 0, "point": 0.0, "interval_95": [0.0, 0.0], "repetitions": repetitions, "seed": seed}
    rng = np.random.default_rng(seed)
    draws = values[rng.integers(0, len(values), size=(repetitions, len(values)))].mean(axis=1)
    return {"families": len(values), "point": float(values.mean()),
            "interval_95": [float(value) for value in np.percentile(draws, [2.5, 97.5])],
            "repetitions": repetitions, "seed": seed}


def gates(metrics: Mapping, *, reload_error: float, paraphrase_consistency: float, latency: Mapping) -> list[dict]:
    values = {"accuracy": metrics["overall"]["accuracy"], "present_accuracy": metrics["present_accuracy"],
              "missing_review_recall": metrics["missing_review_recall"],
              "capability_accuracy": min((row["accuracy"] for row in metrics["breakdowns"]["capability"].values()), default=0.0),
              "coverage": metrics["coverage"], "accepted": metrics["accepted"], "wilson_error_upper": metrics["accepted_wilson_error_upper"],
              "order_consistency": metrics["order_consistency"], "paraphrase_consistency": paraphrase_consistency,
              "reload_max_error": reload_error, "p95_three_seconds": latency.get("p95_3"), "p95_seven_seconds": latency.get("p95_7"),
              "accepted_families": metrics["accepted_families"], "family_wilson_error_upper": metrics["accepted_family_wilson_error_upper"]}
    targets = {**GATES, "accepted_families": 40, "family_wilson_error_upper": .10}
    lower_is_better = {"wilson_error_upper", "reload_max_error", "p95_three_seconds", "p95_seven_seconds", "family_wilson_error_upper"}
    return [{"gate": name, "actual": values.get(name), "target": target,
             "passed": bool(values.get(name) is not None and (values[name] <= target if name in lower_is_better else values[name] >= target))}
            for name, target in targets.items()]


def fail_closed_policy(adapter_sha256: str, domain: str, gates: Sequence[Mapping], *, temperature: float, threshold: float | None,
                       validated_max_tokens: int = 1536, validated_candidate_counts: Sequence[int] = (2, 3, 4, 5, 6, 7)) -> dict:
    passed = all(bool(gate.get("passed")) for gate in gates)
    return {"protocol": "som-v1", "adapter_sha256": adapter_sha256, "acceptance_passed": passed,
            "status": "ready" if passed else "human_review", "domains": {domain: {"temperature": temperature, "threshold": threshold}},
            "validated_max_tokens": validated_max_tokens, "validated_candidate_counts": sorted(set(validated_candidate_counts)),
            "failure_mode": "human_review"}
