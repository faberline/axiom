"""Resumable, input-bound evaluation records for the slower coder model."""
import hashlib
import json

import numpy as np

from ..paths import read_json, write_json


def collect(rows, scorer, destination):
    digest = hashlib.sha256(json.dumps(rows, sort_keys=True, ensure_ascii=False).encode()).hexdigest()
    binding = destination.with_suffix(".inputs.json")
    partial = destination.with_suffix(".partial.json")
    expected = {"rows_sha256": digest, "n": len(rows)}
    if binding.exists() and read_json(binding) != expected:
        raise ValueError(f"Evaluation inputs changed: {destination.name}")
    if not binding.exists() and (destination.exists() or partial.exists()):
        raise ValueError("Evaluation cache has no input provenance.")
    write_json(binding, expected)
    records = read_json(destination if destination.exists() else partial) if destination.exists() or partial.exists() else []
    if len(records) > len(rows):
        raise ValueError("Evaluation cache is longer than its inputs.")
    for row, record in zip(rows, records):
        ids = [c["id"] for c in row["candidates"]]
        if record["id"] != row["id"] or record["candidate_ids"] != ids or ids[record["gold_index"]] != row["gold_candidate_id"]:
            raise ValueError("Evaluation cache does not match candidate identities.")
    if destination.exists():
        if len(records) != len(rows):
            raise ValueError("Completed evaluation cache is incomplete.")
        print(json.dumps({"evaluation": destination.stem, "reused_saved_results": True}), flush=True)
        return records
    if len(records) < len(rows):
        scorer(rows[len(records)])  # Exclude warm-up from latency.
    for row in rows[len(records):]:
        result = scorer(row)
        if len(result["logits"]) != len(row["candidates"]) or not np.isfinite(result["logits"]).all():
            raise RuntimeError("Invalid evaluation logits.")
        records.append({"id": row["id"], "task": row["task"], "family": row.get("family"),
                        "candidate_ids": [c["id"] for c in row["candidates"]],
                        "gold_index": next(i for i,c in enumerate(row["candidates"]) if c["id"] == row["gold_candidate_id"]),
                        **result})
        if len(records) % 32 == 0:
            write_json(partial, records)
            print(json.dumps({"evaluation": destination.stem, "done": len(records), "total": len(rows)}), flush=True)
    write_json(destination, records)
    return records
