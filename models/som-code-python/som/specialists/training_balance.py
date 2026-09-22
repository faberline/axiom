"""Frozen candidate-correctness class balance for repository-owned rows."""
from __future__ import annotations


def candidate_correctness_balance(rows: list[dict]) -> dict:
    """Count exact fixed candidate labels and return a normalized BCE weight.

    Each present row supplies exactly one positive candidate.  A missing-answer
    row supplies none.  The returned positive weight makes the total positive
    BCE mass equal the total negative BCE mass without reading any request or
    environment input.
    """
    positives = 0
    candidates = 0
    for row in rows:
        values = row.get("candidates")
        if not isinstance(values, list) or not values:
            raise ValueError("SOM balance requires fixed rows with candidates.")
        candidates += len(values)
        if not bool(row.get("missing_correct_patch")):
            gold = row.get("gold_candidate_id")
            if not isinstance(gold, str) or sum(candidate.get("id") == gold for candidate in values) != 1:
                raise ValueError("SOM present balance row must contain exactly one gold candidate.")
            positives += 1
    negatives = candidates - positives
    if positives < 1 or negatives < 1:
        raise ValueError("SOM candidate correctness balance requires positive and negative examples.")
    weight = negatives / positives
    return {"positive_candidates": positives, "negative_candidates": negatives,
            "candidate_positive_weight": float(weight),
            "positive_bce_mass": float(positives * weight),
            "negative_bce_mass": float(negatives)}


def feature_cache_binding(context_sha256: str, row_sha256: list[str], rows: list[dict]) -> dict:
    """Return the immutable part of a feature-cache index.

    This stays free of MLX so the cache provenance contract can be checked
    without opening a model device.
    """
    if not isinstance(context_sha256, str) or not context_sha256:
        raise ValueError("SOM feature cache context must be a nonempty string.")
    if not all(isinstance(value, str) and value for value in row_sha256):
        raise ValueError("SOM feature cache row digests must be nonempty strings.")
    return {"context_sha256": context_sha256, "row_sha256": list(row_sha256),
            "candidate_correctness_balance": candidate_correctness_balance(rows)}
