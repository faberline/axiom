"""A single additional epoch from v4, using its unchanged frozen-prefix cache."""
import json
import random
import signal
import time
from pathlib import Path

import mlx.core as mx
import mlx.nn as nn
import mlx.optimizers as optim
import numpy as np
from mlx.utils import tree_flatten, tree_map, tree_unflatten

from ..paths import ROOT, read_json, read_rows, resolve_checkpoint, sha256, write_json
from .modern_assets import LOCK, verify as verify_assets
from .ranker_data import DATA, verify as verify_data
from .ranker_model import CONFIG, load_ranker, restore, save
from .ranker_train import checkpoint, assess, load_features, check_memory, verify_protocol as verify_previous
from .next_loss import LOSS_CONFIG, loss
from .next_holdout import DATA as PUBLIC, rows as public_rows
from .coder_protocol import CRITERIA

RUN = ROOT / "runs/som/research/next"
PREVIOUS = ROOT / "runs/som/research/ranker"
START = PREVIOUS / "checkpoints/001848-9ed1612d"
START_SHA = "cc348baf9551cd3f8d68cb53638644bf05180da88cdd7e590fb677cf44d4b27b"
TRAIN_CONFIG = {"learning_rate": 5e-5, "gradient_accumulation": 8, **LOSS_CONFIG,
                "max_epochs": 1, "max_seconds": 7200, "memory_gib": 24, "seed": 51,
                "max_tokens": 1536, "rank": 8, "scale": 16,
                "initialization": "v4 selected adapter; fresh AdamW optimizer",
                "data_exposure": "One additional pass over the same 1848 training rows; two cumulative epochs including v4."}
BOUND_FILES = ("next_train", "next_loss", "next_holdout", "next_evaluate", "next_predict", "next_diagnostics")


def protocol(run=RUN):
    return {"training": TRAIN_CONFIG, "model": CONFIG,
            "parent_adapter_sha256": START_SHA, "parent_checkpoint": str(START.relative_to(ROOT)),
            "parent_protocol_sha256": sha256(PREVIOUS / "acceptance-protocol.json"),
            "data_sha256": sha256(DATA / "manifest.json"), "source_sha256": sha256(LOCK),
            "public_sha256": sha256(PUBLIC / "manifest.json"),
            "cache_indices": {s: sha256(PREVIOUS / "features" / s / "index.json") for s in ("train", "validation")},
            "code_sha256": {f"som/developer/{p}.py": sha256(ROOT / f"som/developer/{p}.py") for p in BOUND_FILES},
            "criteria": {k: list(v) for k, v in CRITERIA.items()},
            "selection": "Lowest domain-balanced validation NLL among unchanged v4, half epoch and full epoch. Test scores never select weights.",
            "test_policy": "32 unused HumanEvalPack problem IDs; previous 80 IDs and browser diagnostics are regression only.",
            "cache_policy": "Read-only reuse of v4 frozen first-31-layer features. Same model, prompt and rows. Cached teacher priors are unused by the v5 loss.",
            "limits": "v4 test errors informed the objective, so old tests and reused validation are not fresh evidence. No new training examples this round."}


def freeze(run):
    run = Path(run); value = protocol(run); path = run / "acceptance-protocol.json"
    if path.exists() and read_json(path) != value:
        raise ValueError("Frozen v5 protocol changed.")
    write_json(path, value)
    return value


def verify_protocol(run):
    verify_previous(PREVIOUS)
    if sha256(START / "adapter.safetensors") != START_SHA:
        raise ValueError("Parent v4 adapter changed.")
    if read_json(Path(run) / "acceptance-protocol.json") != protocol(run):
        raise ValueError("V5 protocol changed.")


def verify_features():
    """Bind all cache bytes and row positions to v4's completed training record."""
    verify_data(); verify_previous(PREVIOUS)
    state = read_json(PREVIOUS / "training_summary.json")
    if state["status"] != "completed" or not state["features_complete"]:
        raise ValueError("V4 cache is incomplete.")
    counts = {}
    for split in ("train", "validation"):
        directory = PREVIOUS / "features" / split
        index = read_json(directory / "index.json"); values = read_rows(DATA / f"{split}.jsonl")
        if sha256(directory / "index.json") != state["feature_hashes"][split] or len(index) != len(values):
            raise ValueError("Parent feature index changed.")
        for i, row in enumerate(values):
            name = f"{i:06d}.safetensors"; entry = index[name]
            if entry["id"] != row["id"] or entry["sha256"] != sha256(directory / name):
                raise ValueError("Parent feature row or bytes changed.")
        counts[split] = len(values)
    return counts


def train(run=RUN, resume=False):
    started = time.monotonic(); run = Path(run)
    verify_assets(); verify_data(); public_rows()
    if sha256(START / "adapter.safetensors") != START_SHA:
        raise ValueError("V4 initialization changed.")
    frozen = freeze(run)
    state = {"config": TRAIN_CONFIG, "protocol": frozen, "samples_seen": 0, "updates": 0,
             "elapsed_seconds": 0., "history": [], "status": "preparing"}
    if (run / "latest.json").exists():
        if not resume:
            raise ValueError("V5 run exists; use --resume or a new run directory.")
        saved = run / read_json(run / "latest.json")["checkpoint"]
        state = read_json(saved / "trainer.json")
        if state["protocol"] != frozen or state["config"] != TRAIN_CONFIG:
            raise ValueError("V5 resume inputs changed.")
        if state["status"] == "completed":
            print(json.dumps({"status": "completed", "unchanged": True}), flush=True)
            return
    elif resume:
        raise ValueError("No v5 checkpoint to resume.")
    elapsed = state["elapsed_seconds"]
    counts = verify_features()
    rows = read_rows(DATA / "train.jsonl"); val = read_rows(DATA / "validation.jsonl")
    order = list(range(len(rows))); random.Random(TRAIN_CONFIG["seed"]).shuffle(order)
    model, tokenizer = load_ranker(saved if resume else START)
    mx.random.seed(TRAIN_CONFIG["seed"])
    optimizer = optim.AdamW(learning_rate=TRAIN_CONFIG["learning_rate"], weight_decay=0)
    optimizer.init(model.trainable_parameters())
    if resume:
        optimizer.state = tree_unflatten(list(mx.load(str(saved / "optimizer.npz")).items()))
        mx.random.state = [mx.load(str(saved / "rng.npz"))["state"]]
    else:
        save(model, run / "initial")
        if sha256(run / "initial/adapter.safetensors") != START_SHA:
            raise ValueError("V5 initialization is not the exact v4 adapter.")
    stopped = {"value": False}
    def handler(signum, frame):
        stopped["value"] = True
    handlers = {s: signal.signal(s, handler) for s in (signal.SIGINT, signal.SIGTERM)}
    def stop():
        state["elapsed_seconds"] = elapsed + time.monotonic() - started
        return stopped["value"] or state["elapsed_seconds"] >= TRAIN_CONFIG["max_seconds"]
    status = "completed"
    train_dir = PREVIOUS / "features/train"; val_dir = PREVIOUS / "features/validation"
    try:
        if "baseline_validation" not in state:
            metrics = assess(model, val, val_dir, run / "baseline-validation.json")
            state["baseline_validation"] = metrics; stop()
            dest = checkpoint(model, optimizer, run, state)
            write_json(run / "selected.json", {"checkpoint": str(dest.relative_to(run)), "kind": "unchanged_v4",
                                               "validation_nll": metrics["mean_nll"]})
            print(json.dumps({"baseline": {d: m["accuracy"] for d, m in metrics["domains"].items()}, "reused_cache": counts}), flush=True)
        gradient = nn.value_and_grad(model, loss)
        while state["samples_seen"] < len(rows):
            if stop():
                status = "interrupted" if stopped["value"] else "time_limit"; break
            model.train(); count = min(TRAIN_CONFIG["gradient_accumulation"], len(rows) - state["samples_seen"])
            accumulated = None; losses = []
            for offset in range(count):
                i = order[state["samples_seen"] + offset]
                value, grads = gradient(model, load_features(train_dir, i))
                accumulated = grads if accumulated is None else tree_map(lambda a, b: a + b, accumulated, grads)
                mx.eval(value, accumulated); losses.append(value.item())
            grads, norm = optim.clip_grad_norm(tree_map(lambda x: x / count, accumulated), 1.)
            mx.eval(norm)
            if not np.isfinite([*losses, norm.item()]).all():
                raise RuntimeError("Non-finite v5 loss or gradient.")
            optimizer.update(model, grads); mx.eval(model.parameters(), optimizer.state)
            state["samples_seen"] += count; state["updates"] += 1; stop()
            entry = {"samples": state["samples_seen"], "loss": float(np.mean(losses)),
                     "elapsed_seconds": state["elapsed_seconds"], "peak_mlx_gib": check_memory()}
            state["history"].append(entry)
            if state["updates"] % 4 == 0:
                print(json.dumps(entry), flush=True)
            if (state["samples_seen"] >= len(rows) // 2 and not state.get("half_assessed")) or state["samples_seen"] == len(rows):
                state["half_assessed"] = True
                metrics = assess(model, val, val_dir, run / f"validation-{state['samples_seen']:06d}.json")
                state.setdefault("validation", {})[str(state["samples_seen"])] = metrics; stop()
                dest = checkpoint(model, optimizer, run, state)
                if metrics["mean_nll"] < read_json(run / "selected.json")["validation_nll"]:
                    write_json(run / "selected.json", {"checkpoint": str(dest.relative_to(run)), "kind": "continued_v4",
                                                       "validation_nll": metrics["mean_nll"]})
                print(json.dumps({"validation_samples": state["samples_seen"], "mean_nll": metrics["mean_nll"],
                                  "accuracy": {d: m["accuracy"] for d, m in metrics["domains"].items()}}), flush=True)
            elif state["updates"] % 16 == 0:
                checkpoint(model, optimizer, run, state)
            del grads, accumulated; mx.clear_cache()
        stop(); state["status"] = status
        checkpoint(model, optimizer, run, state)
        write_json(run / "training_summary.json", {**state, "peak_mlx_gib": check_memory(),
                   "trainable_parameters": sum(v.size for _, v in tree_flatten(model.trainable_parameters()))})
        print(json.dumps({"status": status, "samples_seen": state["samples_seen"], "elapsed_seconds": state["elapsed_seconds"]}), flush=True)
    finally:
        for s, h in handlers.items():
            signal.signal(s, h)


if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser(); parser.add_argument("--run", type=Path, default=RUN)
    parser.add_argument("--resume", action="store_true")
    args = parser.parse_args(); train(args.run, args.resume)
