"""Bounded domain training. Checkpoint selection uses validation only."""
import copy
import json
import math
import random
import resource
import signal
import time
from pathlib import Path

import mlx.core as mx
import mlx.nn as nn
import mlx.optimizers as optim
import numpy as np
from mlx.utils import tree_flatten, tree_map, tree_unflatten

from ..model import MODEL_CONFIG, load_model, restore_adapter, save_adapter
from ..paths import ROOT, read_json, read_rows, sha256, write_json
from ..schema import encode
from ..train import assess, checkpoint
from .data import DATA, MAX_TOKENS, PROTOCOL, verify

RUN = ROOT / "runs" / "developer-v1"
SMOKE = ROOT / "runs" / "developer-smoke-v1"
CONFIG = {**MODEL_CONFIG, "max_tokens": MAX_TOKENS}


def train(run=RUN, smoke=False, resume=False):
    verify()
    run = Path(run)
    run.mkdir(parents=True, exist_ok=True)
    manifest_hash = sha256(DATA / "manifest.json")
    if not smoke:
        smoke_state = read_json(SMOKE / "training_summary.json")
        if smoke_state["status"] != "smoke_passed" or smoke_state["manifest_sha256"] != manifest_hash:
            raise ValueError("Developer smoke test must pass on the same data before formal training.")
    if (run / "latest.json").exists() and not resume:
        raise ValueError("Developer run already exists. Use --resume or another --run directory.")
    rows = read_rows(DATA / "train.jsonl")
    if smoke:
        rows = sum(([r for r in rows if r["task"] == task][:16] for task in ("python", "frontend")), [])
    validation = rows if smoke else read_rows(DATA / "validation.jsonl")
    config = {"model": CONFIG, "learning_rate": 1e-4, "gradient_accumulation": 8,
              "max_epochs": 12 if smoke else PROTOCOL["max_epochs"], "smoke": smoke,
              "max_seconds": 7200, "clip_norm": 1.0, "seed": PROTOCOL["seed"]}
    state = {"samples_seen": 0, "updates": 0, "elapsed_seconds": 0.0, "history": [],
             "manifest_sha256": manifest_hash, "config": config, "best_loss": None}
    previous = None
    if resume:
        saved = run / read_json(run / "latest.json")["checkpoint"]
        previous = read_json(saved / "trainer.json")
        if previous["config"] != config or previous["manifest_sha256"] != manifest_hash:
            raise ValueError("Cannot resume developer run: configuration or data changed.")
        if previous.get("status") in ("completed", "smoke_passed"):
            print(json.dumps({"status": previous["status"], "unchanged": True}), flush=True)
            return
    model, tokenizer = load_model(CONFIG)
    validation_encoded = [encode(row, tokenizer, MAX_TOKENS) for row in validation]
    optimizer = optim.AdamW(learning_rate=config["learning_rate"], weight_decay=0)
    optimizer.init(model.trainable_parameters())
    if previous:
        restore_adapter(model, saved)
        optimizer.state = tree_unflatten(list(mx.load(str(saved / "optimizer.npz")).items()))
        mx.random.state = [mx.load(str(saved / "rng.npz"))["state"]]
        state = previous
    else:
        save_adapter(model, run / "initial")
        state["initial_validation"] = assess(model, validation_encoded, validation)
        checkpoint(model, optimizer, run, state)
        print(json.dumps({"initial_validation": state["initial_validation"]}), flush=True)

    def loss(current, item, gold):
        logits = current(mx.array([item.tokens]), item.candidate_positions, item.decision_position)
        return nn.losses.cross_entropy(logits, mx.array([gold]), reduction="mean")
    value_and_grad = nn.value_and_grad(model, loss)
    stop = {"requested": False}
    def handler(signum, frame):
        stop["requested"] = True
    handlers = {s: signal.signal(s, handler) for s in (signal.SIGINT, signal.SIGTERM)}
    started, elapsed_before = time.monotonic(), state["elapsed_seconds"]
    planned = len(rows) * config["max_epochs"]
    status = "completed"
    try:
        while state["samples_seen"] < planned:
            if stop["requested"]:
                status = "interrupted"
                break
            if elapsed_before + time.monotonic() - started >= config["max_seconds"]:
                status = "time_limit"
                break
            if resource.getrusage(resource.RUSAGE_SELF).ru_maxrss > 24 * 1024**3:
                status = "process_memory_limit"
                break
            model.train()
            epoch, position = divmod(state["samples_seen"], len(rows))
            order = list(range(len(rows)))
            random.Random(config["seed"] + epoch).shuffle(order)
            count = min(8, len(rows) - position, planned - state["samples_seen"])
            grads_sum, losses = None, []
            for offset in range(count):
                row = copy.deepcopy(rows[order[position + offset]])
                if not smoke:
                    random.Random(config["seed"] + state["samples_seen"] + offset).shuffle(row["candidates"])
                item = encode(row, tokenizer, MAX_TOKENS)
                gold = next(i for i, c in enumerate(row["candidates"]) if c["id"] == row["gold_candidate_id"])
                value, grads = value_and_grad(model, item, gold)
                grads_sum = grads if grads_sum is None else tree_map(lambda a,b: a+b, grads_sum, grads)
                mx.eval(value, grads_sum)
                losses.append(value.item())
            grads = tree_map(lambda x: x/count, grads_sum)
            grads, norm = optim.clip_grad_norm(grads, config["clip_norm"])
            mx.eval(norm)
            if not all(math.isfinite(x) for x in [*losses, norm.item()]):
                raise RuntimeError("Non-finite training value; last complete checkpoint is preserved.")
            optimizer.update(model, grads)
            mx.eval(model.parameters(), optimizer.state)
            state["samples_seen"] += count
            state["updates"] += 1
            state["elapsed_seconds"] = elapsed_before + time.monotonic() - started
            record = {"update": state["updates"], "samples": state["samples_seen"],
                      "loss": float(np.mean(losses)), "elapsed_seconds": state["elapsed_seconds"],
                      "peak_mlx_gib": mx.get_peak_memory()/1024**3,
                      "peak_process_rss_gib": resource.getrusage(resource.RUSAGE_SELF).ru_maxrss/1024**3}
            state["history"].append(record)
            if state["updates"] % 25 == 0:
                print(json.dumps(record), flush=True)
            if state["samples_seen"] % len(rows) == 0:
                validation_result = assess(model, validation_encoded, validation)
                state["last_validation"] = validation_result
                print(json.dumps({"epoch": epoch + 1, "validation": validation_result}), flush=True)
                better = state["best_loss"] is None or validation_result["loss"] < state["best_loss"]
                if better:
                    state["best_loss"] = validation_result["loss"]
                destination = checkpoint(model, optimizer, run, state)
                if better:
                    write_json(run / "selected.json", {"checkpoint": str(destination.relative_to(run)),
                               "reason": "Lowest validation NLL among completed epochs", "validation": validation_result})
                if smoke and validation_result["accuracy"] >= .90 and validation_result["loss"] < state["initial_validation"]["loss"]:
                    status = "smoke_passed"
                    break
            elif state["updates"] % 100 == 0:
                checkpoint(model, optimizer, run, state)
            del grads, grads_sum
            if mx.get_cache_memory() > 256 * 1024**2:
                mx.clear_cache()
        if smoke and status == "completed":
            status = "smoke_failed"
        state["elapsed_seconds"] = elapsed_before + time.monotonic() - started
        state["status"] = status
        destination = checkpoint(model, optimizer, run, state)
        write_json(run / "training_summary.json", {**state, "last_checkpoint": str(destination.relative_to(run)),
                   "trainable_parameters": sum(v.size for _,v in tree_flatten(model.trainable_parameters())),
                   "planned_samples": planned, "unique_training_instances": len(rows),
                   "peak_mlx_gib": mx.get_peak_memory()/1024**3,
                   "peak_process_rss_gib": resource.getrusage(resource.RUSAGE_SELF).ru_maxrss/1024**3})
        print(json.dumps({"status": status, "samples_seen": state["samples_seen"], "run": str(run)}), flush=True)
        if status == "smoke_failed":
            raise RuntimeError("Developer smoke test did not learn. Formal training is blocked.")
    finally:
        for s,h in handlers.items():
            signal.signal(s,h)
