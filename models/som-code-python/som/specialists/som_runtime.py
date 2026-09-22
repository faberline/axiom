"""Frozen-prefix feature cache used only by SOM specialist training.

This module receives repository-owned corpus rows.  It never accepts a public
prediction request and it never executes supplied candidate text.
"""
from __future__ import annotations

import hashlib
import json
from pathlib import Path

import mlx.core as mx
import numpy as np

from ..paths import read_json, sha256, write_json
from .input import parse, prompt, scores_with_review, stable_scores
from .loss import specialist_loss
from .som_config import TRAIN_CONFIG
from .som_metrics import smoke_metric_breakdown
from .training_balance import candidate_correctness_balance, feature_cache_binding


def _row_hash(row: dict) -> str:
    return hashlib.sha256(
        json.dumps(row, sort_keys=True, ensure_ascii=False, separators=(",", ":")).encode()
    ).hexdigest()


def _canonical_row(row: dict):
    request = parse({key: row[key] for key in ("state", "question", "candidates")})
    candidates = sorted(request.candidates, key=lambda candidate: (candidate.text, candidate.id))
    missing = bool(row["missing_correct_patch"])
    if missing:
        gold = len(candidates)
    else:
        gold_id = row.get("gold_candidate_id")
        gold = next((index for index, candidate in enumerate(candidates) if candidate.id == gold_id), None)
        if gold is None:
            raise ValueError("SOM present training row has no supplied gold candidate.")
    return request, candidates, gold, missing


def _features_for_row(model, tokenizer, row: dict, *, candidate_positive_weight: float) -> dict:
    request, candidates, gold, missing = _canonical_row(row)
    values, priors, labels = {}, [], None
    for index, candidate in enumerate(candidates):
        tokens, candidate_labels, candidate_end = prompt(request, candidate, tokenizer, include_candidate_boundary=True)
        if labels is None:
            labels = candidate_labels
        elif labels != candidate_labels:
            raise ValueError("SOM verifier labels differ between candidates.")
        hidden = model.prefix_features(mx.array([tokens]))
        logits = model.score_features(hidden, labels, candidate_end)[0]
        mx.eval(hidden, logits)
        values[f"hidden{index}"] = hidden
        values[f"candidate_end{index}"] = mx.array(candidate_end)
        priors.append(float((logits[0] - logits[1]).item()))
    values.update({
        "prior": mx.array(priors),
        "labels": mx.array(labels),
        "count": mx.array(len(candidates)),
        "gold": mx.array(gold),
        "missing": mx.array(int(missing)),
        "candidate_positive_weight": mx.array(candidate_positive_weight),
    })
    return values


def _cache_features(model, tokenizer, rows: list[dict], directory: Path, context: str, stop) -> str | None:
    """Cache detached frozen-prefix outputs and bind each file to its source row."""
    directory.mkdir(parents=True, exist_ok=True)
    index_path = directory / "index.json"
    expected_rows = [_row_hash(row) for row in rows]
    binding = feature_cache_binding(context, expected_rows, rows)
    balance = binding["candidate_correctness_balance"]
    if index_path.exists():
        index = read_json(index_path)
        if any(index.get(key) != value for key, value in binding.items()):
            raise ValueError("SOM frozen feature cache provenance differs from this run.")
    else:
        index = {**binding, "features": {}}
    features = index.setdefault("features", {})
    for position, row_digest in enumerate(expected_rows):
        name = f"{position:06d}.safetensors"
        destination = directory / name
        entry = features.get(name)
        if entry is not None:
            if entry != {"row_sha256": row_digest, "sha256": sha256(destination)}:
                raise ValueError("SOM frozen feature cache changed.")
            continue
        if stop():
            return None
        values = _features_for_row(model, tokenizer, rows[position],
                                   candidate_positive_weight=balance["candidate_positive_weight"])
        mx.eval(values)
        if not np.isfinite(values["prior"].tolist()).all():
            raise RuntimeError("SOM teacher scores are non-finite.")
        temporary = directory / f"temporary-{name}"
        mx.save_safetensors(str(temporary), values)
        temporary.replace(destination)
        features[name] = {"row_sha256": row_digest, "sha256": sha256(destination)}
        write_json(index_path, index)
        mx.clear_cache()
        _check_memory()
    if len(features) != len(rows):
        raise ValueError("SOM frozen feature cache length changed.")
    write_json(index_path, index)
    return sha256(index_path)


def _load_feature(directory: Path, index: int):
    return mx.load(str(directory / f"{index:06d}.safetensors"))


def _loss(model, features):
    count = int(features["count"].item())
    scores = mx.stack([
        model.score_features(features[f"hidden{index}"], features["labels"], features[f"candidate_end{index}"])[0] @ mx.array([1.0, -1.0])
        for index in range(count)
    ])
    return specialist_loss(
        scores,
        int(features["gold"].item()),
        bool(features["missing"].item()),
        features["prior"], features["candidate_positive_weight"],
    )


def _metrics(model, tokenizer, rows: list[dict], directory: Path) -> dict:
    del tokenizer
    model.eval()
    losses, correct = [], 0
    present_correct = present_rows = 0
    missing_review_correct = missing_rows = 0
    for index, _ in enumerate(rows):
        features = _load_feature(directory, index)
        count = int(features["count"].item())
        scores = mx.stack([
            model.score_features(features[f"hidden{candidate}"], features["labels"], features[f"candidate_end{candidate}"])[0] @ mx.array([1.0, -1.0])
            for candidate in range(count)
        ])
        loss = _loss(model, features)
        mx.eval(scores, loss)
        _, candidates, _, _ = _canonical_row(rows[index])
        raw = [float(value) for value in scores.tolist()]
        candidate_scores = stable_scores(candidates, raw)
        logits = scores_with_review(candidates, candidate_scores)
        predicted = int(np.argmax(logits))
        gold = int(features["gold"].item())
        is_missing = bool(features["missing"].item())
        row_correct = predicted == gold
        correct += int(row_correct)
        if is_missing:
            missing_rows += 1
            missing_review_correct += int(row_correct)
        else:
            present_rows += 1
            present_correct += int(row_correct)
        losses.append(float(loss.item()))
    return smoke_metric_breakdown(
        loss=float(np.mean(losses)), correct=correct, rows=len(rows),
        present_correct=present_correct, present_rows=present_rows,
        missing_review_correct=missing_review_correct, missing_rows=missing_rows,
    )


def _check_memory() -> float:
    peak = mx.get_peak_memory() / 1024**3
    if peak > TRAIN_CONFIG["memory_gib"]:
        raise RuntimeError(f"SOM MLX memory budget exceeded: {peak:.2f} GiB.")
    return peak
