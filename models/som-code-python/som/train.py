import json
import math
import signal
import time
import uuid
from pathlib import Path

import mlx.core as mx
import mlx.nn as nn
import mlx.optimizers as optim
import numpy as np
from mlx.utils import tree_flatten, tree_map, tree_unflatten

from .model import load_model, encoded_logits, save_adapter, restore_adapter
from .paths import ROOT, read_rows, write_json, read_json, sha256, resolve_checkpoint, verify_data
from .schema import encode

TRAIN_CONFIG = {"learning_rate": 1e-4, "gradient_accumulation": 8,
                "batch_size": 1, "max_tokens": 512, "clip_norm": 1.0,
                "max_seconds": 7200, "memory_gib": 24, "seed": 42}


def checkpoint(model, optimizer, run, state):
    name = f"checkpoints/{state['samples_seen']:06d}-{uuid.uuid4().hex[:8]}"
    destination = run / name
    save_adapter(model, destination)
    mx.eval(optimizer.state)
    mx.savez(str(destination / "optimizer.npz"), **dict(tree_flatten(optimizer.state)))
    mx.savez(str(destination / "rng.npz"), state=mx.random.state[0])
    write_json(destination / "trainer.json", state)
    # Publish the pointer only after every file is written.
    write_json(run / "latest.json", {"checkpoint": name})
    return destination


def assess(model, encoded, rows):
    model.eval()
    losses, correct = [], 0
    for item, row in zip(encoded, rows):
        z = encoded_logits(model, item).astype(mx.float32)
        gold = next(i for i, c in enumerate(row["candidates"]) if c["id"] == row["gold_candidate_id"])
        losses.append(float((mx.logsumexp(z) - z[gold]).item()))
        correct += int(z.argmax().item()) == gold
    return {"n": len(rows), "loss": float(np.mean(losses)), "accuracy": correct / len(rows)}


def train(run, smoke=False, resume=False):
    verify_data()
    if not smoke:
        smoke_result = ROOT / "runs" / "smoke" / "training_summary.json"
        if not smoke_result.exists() or read_json(smoke_result).get("status") != "smoke_passed":
            raise ValueError("Run som smoke successfully before formal training.")
    run = Path(run)
    run.mkdir(parents=True, exist_ok=True)
    if (run / "latest.json").exists() and not resume:
        raise ValueError("Run already exists. Use --resume or choose another run directory.")
    if resume and not (run / "latest.json").exists():
        raise ValueError("No saved checkpoint exists to resume.")
    if resume:
        previous = read_json(resolve_checkpoint(run) / "trainer.json")
        if previous.get("status") in ("completed", "smoke_passed"):
            print(json.dumps({"status": previous["status"], "message": "This run is already complete; no parameters changed."}), flush=True)
            return
    train_file = ROOT / "data" / "train.jsonl"
    rows = read_rows(train_file)
    if smoke:
        # Equal task counts, from training data only.
        rows = sum(([r for r in rows if r["task"] == task][:16] for task in ("banking77", "emotion")), [])
    model, tokenizer = load_model()
    encoded = [encode(r, tokenizer) for r in rows]
    val_rows = rows if smoke else read_rows(ROOT / "data" / "validation.jsonl")
    val_encoded = [encode(r, tokenizer) for r in val_rows]
    optimizer = optim.AdamW(learning_rate=TRAIN_CONFIG["learning_rate"], weight_decay=0.0)
    optimizer.init(model.trainable_parameters())
    config = {**TRAIN_CONFIG, "smoke": smoke, "max_epochs": 10 if smoke else 1}
    state = {"config": config, "samples_seen": 0, "updates": 0, "elapsed_seconds": 0.0,
             "training_data_sha256": sha256(train_file),
             "manifest_sha256": sha256(ROOT / "data" / "manifest.json"), "history": []}
    if resume:
        saved = resolve_checkpoint(run)
        previous = read_json(saved / "trainer.json")
        for key in ("config", "training_data_sha256", "manifest_sha256"):
            if previous[key] != state[key]:
                raise ValueError(f"Cannot resume: {key} changed.")
        restore_adapter(model, saved)
        optimizer.state = tree_unflatten(list(mx.load(str(saved / "optimizer.npz")).items()))
        mx.random.state = [mx.load(str(saved / "rng.npz"))["state"]]
        state = previous
    else:
        save_adapter(model, run / "initial")
        state["initial_validation"] = assess(model, val_encoded, val_rows)
        checkpoint(model, optimizer, run, state)
        print(json.dumps({"initial_validation": state["initial_validation"]}), flush=True)

    def loss_fn(current_model, tokens, positions, final_position, gold):
        logits = current_model(tokens, positions, final_position).astype(mx.float32)
        return nn.losses.cross_entropy(logits, mx.array([gold]), reduction="mean")

    value_and_grad = nn.value_and_grad(model, loss_fn)
    stopping = {"requested": False}
    def stop_handler(signum, frame):
        stopping["requested"] = True
    old_handlers = {s: signal.signal(s, stop_handler) for s in (signal.SIGINT, signal.SIGTERM)}
    started = time.monotonic()
    prior_elapsed = state["elapsed_seconds"]
    total = len(rows) * config["max_epochs"]
    status = "completed"
    try:
        model.train()
        while state["samples_seen"] < total:
            if stopping["requested"]:
                status = "interrupted"
                break
            if prior_elapsed + time.monotonic() - started >= config["max_seconds"]:
                status = "time_limit"
                break
            count = min(config["gradient_accumulation"], total - state["samples_seen"])
            accumulated = None
            losses = []
            token_count = 0
            for offset in range(count):
                index = (state["samples_seen"] + offset) % len(rows)
                item, row = encoded[index], rows[index]
                gold = next(i for i, c in enumerate(row["candidates"]) if c["id"] == row["gold_candidate_id"])
                value, grads = value_and_grad(model, mx.array([item.tokens]), item.candidate_positions, item.decision_position, gold)
                accumulated = grads if accumulated is None else tree_map(lambda a, b: a + b, accumulated, grads)
                mx.eval(value, accumulated)
                losses.append(float(value.item()))
                token_count += len(item.tokens)
            if not all(math.isfinite(v) for v in losses):
                raise RuntimeError("Non-finite loss. Last published checkpoint remains usable.")
            grads = tree_map(lambda x: x / count, accumulated)
            grads, norm = optim.clip_grad_norm(grads, config["clip_norm"])
            mx.eval(norm)
            if not math.isfinite(float(norm.item())):
                raise RuntimeError("Non-finite gradient. Last published checkpoint remains usable.")
            optimizer.update(model, grads)
            mx.eval(model.parameters(), optimizer.state)
            state["samples_seen"] += count
            state["updates"] += 1
            state["elapsed_seconds"] = prior_elapsed + time.monotonic() - started
            record = {"update": state["updates"], "samples": state["samples_seen"],
                      "loss": float(np.mean(losses)), "gradient_norm": float(norm.item()),
                      "elapsed_seconds": state["elapsed_seconds"], "tokens": token_count,
                      "peak_memory_gib": mx.get_peak_memory() / 1024**3}
            state["history"].append(record)
            if state["updates"] % 10 == 0:
                print(json.dumps(record), flush=True)
                checkpoint(model, optimizer, run, state)
            if smoke and state["samples_seen"] % len(rows) == 0:
                check = assess(model, val_encoded, val_rows)
                print(json.dumps({"smoke_epoch": state["samples_seen"] // len(rows), **check}), flush=True)
                model.train()
                if check["accuracy"] >= 0.90 and check["loss"] < state["initial_validation"]["loss"]:
                    status = "smoke_passed"
                    break
            del accumulated, grads
            if mx.get_cache_memory() > 256 * 1024**2:
                mx.clear_cache()
        state["elapsed_seconds"] = prior_elapsed + time.monotonic() - started
        final = assess(model, val_encoded, val_rows)
        state["final_validation"] = final
        if smoke and status == "completed":
            status = "smoke_failed"
        state["status"] = status
        destination = checkpoint(model, optimizer, run, state)
        write_json(run / "training_summary.json", {**state, "checkpoint": str(destination.relative_to(run)),
                   "trainable_parameters": sum(v.size for _, v in tree_flatten(model.trainable_parameters())),
                   "total_training_examples": len(rows), "planned_samples": total,
                   "peak_memory_gib": mx.get_peak_memory() / 1024**3})
        print(json.dumps({"status": status, "samples_seen": state["samples_seen"], "validation": final,
                          "checkpoint": str(destination)}), flush=True)
        if status == "smoke_failed":
            raise RuntimeError("32-example learning test did not pass. Do not start the full run.")
    finally:
        for s, handler in old_handlers.items():
            signal.signal(s, handler)
