"""MLX LoRA Training Pipeline for Structured Outcome Models (SOM).

Accepts parsed datasets from som_core.dataset (including python-v2 scenarios),
supports both real pre-trained base models and self-contained MLX LoRA skeleton
backbones for deterministic local training on Apple Silicon.
"""

from __future__ import annotations

import json
import math
import os
from pathlib import Path
import signal
import time
from typing import Any, Optional
import uuid

import mlx.core as mx
import mlx.nn as nn
import mlx.optimizers as optim
from mlx.utils import tree_flatten, tree_map, tree_unflatten
import numpy as np

from .dataset import find_python_v2_dir, load_python_v2_dataset, split_dataset
from .model import DecisionModel, encoded_logits, load_model, restore_adapter, save_adapter
from .paths import ROOT, model_path, read_json, read_rows, resolve_checkpoint, sha256, write_json
from .schema import encode

TRAIN_CONFIG = {
    "learning_rate": 1e-4,
    "gradient_accumulation": 4,
    "batch_size": 1,
    "max_tokens": 16384,
    "clip_norm": 1.0,
    "max_seconds": 7200,
    "memory_gib": 24,
    "seed": 42,
    "rank": 8,
    "scale": 16.0,
    "projection_dim": 256,
}


class SkeletonAttention(nn.Module):
    """Causal self-attention layer for the skeleton backbone."""

    def __init__(self, hidden_size: int):
        super().__init__()
        self.q_proj = nn.Linear(hidden_size, hidden_size, bias=False)
        self.k_proj = nn.Linear(hidden_size, hidden_size, bias=False)
        self.v_proj = nn.Linear(hidden_size, hidden_size, bias=False)
        self.out_proj = nn.Linear(hidden_size, hidden_size, bias=False)
        self.scale = 1.0 / math.sqrt(hidden_size)

    def __call__(self, x: mx.array) -> mx.array:
        B, L, D = x.shape
        q = self.q_proj(x)
        k = self.k_proj(x)
        v = self.v_proj(x)
        scores = (q @ k.transpose(0, 2, 1)) * self.scale
        mask = nn.MultiHeadAttention.create_additive_causal_mask(L).astype(x.dtype)
        weights = mx.softmax(scores + mask, axis=-1)
        return self.out_proj(weights @ v)


class SkeletonDecoderLayer(nn.Module):
    """Decoder layer with attention and residual connection."""

    def __init__(self, hidden_size: int):
        super().__init__()
        self.self_attn = SkeletonAttention(hidden_size)

    def __call__(self, x: mx.array) -> mx.array:
        return x + self.self_attn(x)


class SkeletonBackboneArgs:
    """Configuration args mimicking transformer backbone."""

    def __init__(self, hidden_size: int = 64, num_layers: int = 2):
        self.hidden_size = hidden_size
        self.num_hidden_layers = num_layers


class SkeletonBackbone(nn.Module):
    """Self-contained MLX backbone for skeleton LoRA training."""

    def __init__(self, vocab_size: int = 32000, hidden_size: int = 64, num_layers: int = 2):
        super().__init__()
        self.args = SkeletonBackboneArgs(hidden_size, num_layers)
        self.embed_tokens = nn.Embedding(vocab_size, hidden_size)
        self.layers = [SkeletonDecoderLayer(hidden_size) for _ in range(num_layers)]

    def __call__(self, tokens: mx.array) -> mx.array:
        x = self.embed_tokens(tokens)
        for layer in self.layers:
            x = layer(x)
        return x


class SkeletonTokenizer:
    """Deterministic, zero-dependency offline tokenizer for skeleton MLX training."""

    def __init__(self, vocab_size: int = 32000):
        self.vocab_size = vocab_size

    def encode(self, text: str, add_special_tokens: bool = False) -> list[int]:
        import hashlib
        import re

        tokens: list[int] = []
        for token in re.findall(r"\w+|[^\w\s]", text, re.UNICODE):
            h = int(hashlib.md5(token.encode("utf-8")).hexdigest()[:8], 16)
            tokens.append((h % (self.vocab_size - 2)) + 1)
        return tokens or [1]

    def decode(self, tokens: list[int]) -> str:
        return " ".join(f"tok_{t}" for t in tokens)


def create_skeleton_model(config: dict[str, Any]) -> tuple[DecisionModel, SkeletonTokenizer]:
    """Create a DecisionModel with SkeletonBackbone and MLX LoRA adapters."""
    hidden_size = 64
    backbone = SkeletonBackbone(vocab_size=32000, hidden_size=hidden_size, num_layers=2)
    model_config = {
        "rank": config.get("rank", 4),
        "scale": config.get("scale", 8.0),
        "projection_dim": config.get("projection_dim", 32),
        "max_tokens": config.get("max_tokens", 16384),
        "seed": config.get("seed", 42),
    }
    model = DecisionModel(backbone, config=model_config)
    tokenizer = SkeletonTokenizer(vocab_size=32000)
    mx.eval(model.parameters())
    return model, tokenizer


def load_training_model(
    config: dict[str, Any],
    force_skeleton: bool = False,
) -> tuple[DecisionModel, Any]:
    """Load model, using real backbone if available or falling back to skeleton."""
    if not force_skeleton:
        try:
            m_path = model_path()
            if m_path.exists() and (m_path / "config.json").exists():
                return load_model(config)
        except Exception as e:
            print(f"[som-train] Base model not loaded ({e}). Falling back to MLX Skeleton backbone.")

    print("[som-train] Using MLX Skeleton LoRA Backbone for training.")
    return create_skeleton_model(config)


def checkpoint(model: DecisionModel, optimizer: optim.Optimizer, run: Path, state: dict[str, Any]) -> Path:
    """Save an adapter checkpoint and optimizer/RNG state."""
    name = f"checkpoints/{state['samples_seen']:06d}-{uuid.uuid4().hex[:8]}"
    destination = run / name
    destination.mkdir(parents=True, exist_ok=True)
    save_adapter(model, destination)
    mx.eval(optimizer.state)
    mx.savez(str(destination / "optimizer.npz"), **dict(tree_flatten(optimizer.state)))
    mx.savez(str(destination / "rng.npz"), state=mx.random.state[0])
    write_json(destination / "trainer.json", state)
    write_json(run / "latest.json", {"checkpoint": name})
    return destination


def assess(
    model: DecisionModel,
    encoded: list[Any],
    rows: list[dict[str, Any]],
) -> dict[str, float | int]:
    """Assess model accuracy and cross-entropy loss over a dataset split."""
    model.eval()
    losses, correct = [], 0
    for item, row in zip(encoded, rows):
        z = encoded_logits(model, item).astype(mx.float32)
        gold = next(i for i, c in enumerate(row["candidates"]) if c["id"] == row["gold_candidate_id"])
        losses.append(float((mx.logsumexp(z) - z[gold]).item()))
        correct += int(z.argmax().item()) == gold
    n = len(rows)
    return {
        "n": n,
        "loss": float(np.mean(losses)) if losses else 0.0,
        "accuracy": (correct / n) if n > 0 else 0.0,
    }


def train(
    run: Path | str = "runs/default",
    smoke: bool = False,
    resume: bool = False,
    dataset: list[dict[str, Any]] | None = None,
    data_dir: Path | str | None = None,
    epochs: int | None = None,
    batch_size: int = 1,
    lr: float = 1e-4,
    use_skeleton: bool = False,
) -> dict[str, Any]:
    """Execute the MLX LoRA training loop.

    Parameters
    ----------
    run : Path | str
        Output run directory for checkpoints and logs.
    smoke : bool
        If True, run minimal smoke test steps.
    resume : bool
        If True, resume from existing checkpoint in run dir.
    dataset : list[dict] | None
        Pre-parsed dataset rows. If None, loaded from data_dir or python-v2.
    data_dir : Path | str | None
        Path to dataset directory if dataset is None.
    epochs : int | None
        Number of epochs (defaults to 1 for smoke, 5 for normal).
    batch_size : int
        Gradient accumulation batch size.
    lr : float
        Learning rate for AdamW.
    use_skeleton : bool
        Force usage of the MLX Skeleton LoRA model.

    Returns
    -------
    dict[str, Any]
        Training summary with metrics, status, and checkpoint location.
    """
    run = Path(run).resolve()
    run.mkdir(parents=True, exist_ok=True)

    if (run / "latest.json").exists() and not resume:
        raise ValueError("Run already exists. Use --resume or choose another run directory.")
    if resume and not (run / "latest.json").exists():
        raise ValueError("No saved checkpoint exists to resume.")

    # 1. Load Dataset
    if dataset is not None:
        rows = list(dataset)
    else:
        print(f"[som-train] Loading dataset from: {data_dir or 'python-v2'}")
        rows = load_python_v2_dataset(data_dir=data_dir)

    if not rows:
        raise ValueError("Dataset is empty. Ensure python-v2 materials exist.")

    print(f"[som-train] Loaded {len(rows)} scenario families for training.")

    train_rows, val_rows = split_dataset(rows, val_ratio=0.25 if len(rows) > 3 else 0.5)
    if smoke:
        train_rows = train_rows[:4]
        val_rows = val_rows[:2]

    # 2. Configure training
    max_epochs = epochs if epochs is not None else (2 if smoke else 5)
    config = {
        **TRAIN_CONFIG,
        "learning_rate": lr,
        "smoke": smoke,
        "max_epochs": max_epochs,
        "gradient_accumulation": min(batch_size, len(train_rows)),
    }

    # 3. Load Model and Tokenizer
    model, tokenizer = load_training_model(config, force_skeleton=use_skeleton)

    # 4. Tokenize & Encode
    max_tokens = config["max_tokens"]
    encoded_train = [encode(r, tokenizer, max_tokens=max_tokens) for r in train_rows]
    encoded_val = [encode(r, tokenizer, max_tokens=max_tokens) for r in val_rows]

    # 5. Optimizer
    optimizer = optim.AdamW(learning_rate=config["learning_rate"], weight_decay=0.0)
    optimizer.init(model.trainable_parameters())

    state = {
        "config": config,
        "samples_seen": 0,
        "updates": 0,
        "elapsed_seconds": 0.0,
        "total_training_examples": len(train_rows),
        "history": [],
    }

    if resume:
        saved = resolve_checkpoint(run)
        previous = read_json(saved / "trainer.json")
        restore_adapter(model, saved)
        optimizer.state = tree_unflatten(list(mx.load(str(saved / "optimizer.npz")).items()))
        mx.random.state = [mx.load(str(saved / "rng.npz"))["state"]]
        state = previous
    else:
        init_val = assess(model, encoded_val, val_rows)
        state["initial_validation"] = init_val
        print(f"[som-train] Initial Validation: loss={init_val['loss']:.4f}, acc={init_val['accuracy']:.2%}")
        checkpoint(model, optimizer, run, state)

    # 6. Loss Function
    def loss_fn(current_model: DecisionModel, tokens: mx.array, positions: list[int], final_pos: int, gold_idx: int) -> mx.array:
        logits = current_model(tokens, positions, final_pos).astype(mx.float32)
        return nn.losses.cross_entropy(logits, mx.array([gold_idx]), reduction="mean")

    value_and_grad = nn.value_and_grad(model, loss_fn)

    # Signal handling for clean interrupts
    stopping = {"requested": False}

    def stop_handler(signum: int, frame: Any) -> None:
        stopping["requested"] = True

    old_handlers = {s: signal.signal(s, stop_handler) for s in (signal.SIGINT, signal.SIGTERM)}

    started = time.monotonic()
    prior_elapsed = state["elapsed_seconds"]
    total_samples = len(train_rows) * config["max_epochs"]
    status = "completed"

    try:
        model.train()
        while state["samples_seen"] < total_samples:
            if stopping["requested"]:
                status = "interrupted"
                break
            if prior_elapsed + time.monotonic() - started >= config["max_seconds"]:
                status = "time_limit"
                break

            count = min(config["gradient_accumulation"], total_samples - state["samples_seen"])
            accumulated = None
            losses: list[float] = []
            token_count = 0

            for offset in range(count):
                idx = (state["samples_seen"] + offset) % len(train_rows)
                item, row = encoded_train[idx], train_rows[idx]
                gold = next(i for i, c in enumerate(row["candidates"]) if c["id"] == row["gold_candidate_id"])

                value, grads = value_and_grad(
                    model,
                    mx.array([item.tokens]),
                    item.candidate_positions,
                    item.decision_position,
                    gold,
                )
                accumulated = grads if accumulated is None else tree_map(lambda a, b: a + b, accumulated, grads)
                mx.eval(value, accumulated)
                losses.append(float(value.item()))
                token_count += len(item.tokens)

            if not all(math.isfinite(v) for v in losses):
                raise RuntimeError("Non-finite loss encountered during training.")

            grads = tree_map(lambda x: x / count, accumulated)
            grads, norm = optim.clip_grad_norm(grads, config["clip_norm"])
            mx.eval(norm)

            optimizer.update(model, grads)
            mx.eval(model.parameters(), optimizer.state)

            state["samples_seen"] += count
            state["updates"] += 1
            state["elapsed_seconds"] = prior_elapsed + time.monotonic() - started

            mean_loss = float(np.mean(losses))
            grad_norm = float(norm.item())
            record = {
                "update": state["updates"],
                "samples": state["samples_seen"],
                "loss": mean_loss,
                "gradient_norm": grad_norm,
                "elapsed_seconds": state["elapsed_seconds"],
                "tokens": token_count,
            }
            state["history"].append(record)

            if state["updates"] % 5 == 0 or state["samples_seen"] >= total_samples:
                print(
                    f"[som-train] Update {state['updates']:03d} | "
                    f"Samples {state['samples_seen']:03d}/{total_samples:03d} | "
                    f"Loss {mean_loss:.4f} | GradNorm {grad_norm:.4f}"
                )

            del accumulated, grads

        state["elapsed_seconds"] = prior_elapsed + time.monotonic() - started
        final_val = assess(model, encoded_val, val_rows)
        state["final_validation"] = final_val
        state["status"] = status

        destination = checkpoint(model, optimizer, run, state)
        trainable_param_count = sum(v.size for _, v in tree_flatten(model.trainable_parameters()))

        summary = {
            **state,
            "checkpoint": str(destination),
            "trainable_parameters": trainable_param_count,
            "planned_samples": total_samples,
        }
        write_json(run / "training_summary.json", summary)

        print(
            f"[som-train] {status.capitalize()} | "
            f"Samples seen: {state['samples_seen']} | "
            f"Final Val Loss: {final_val['loss']:.4f}, Acc: {final_val['accuracy']:.2%} | "
            f"Checkpoint: {destination.name}"
        )
        return summary

    finally:
        for s, handler in old_handlers.items():
            signal.signal(s, handler)
