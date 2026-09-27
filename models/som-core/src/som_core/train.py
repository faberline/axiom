"""Layer SFT: LoRA next-token training of the L1, L2, and L3 layers.

Rules this module owns:

- The base model is the one ``sources.lock.json`` names at its pinned
  ``revision``; every locked file is downloaded and its sha256 compared with
  the lock before a weight is loaded, and a mismatch or a missing lock is
  refused. :func:`write_lock` is the only writer of the lock.
- Each family row yields one pair per layer (:data:`LAYERS`): ``l1`` maps
  the caption to the L1 ``plan``, ``l2`` to ``decompiled.topology``, and
  ``l3`` to ``decompiled.ops``. Conditioning is cumulative
  (:func:`layer_prompt`): every layer's prompt carries the caption and every
  upstream record, because the topology alone names blocks, not the
  signatures and raise conditions the ops' literal source must implement.
  Records are compact JSON; a row missing a record yields no pair for the
  layers that need it. ``training_summary.json`` records the
  ``conditioning`` so ``som eval`` rebuilds the prompts the adapter saw;
  a summary without it predates this rule (``previous``: each layer saw
  only the record above it).
- The ``l3`` completion is written in :data:`L3_FORMAT`: ``fenced``
  (:func:`som_core.wire.dump_ops`, each ``INSERT_BLOCK`` source verbatim in a
  code fence) with its own system line; ``training_summary.json`` records
  ``l3_format``, and a summary without it predates this rule (``json``: the
  operation list as one compact JSON array under the original system line,
  which :data:`SYSTEM` keeps byte-for-byte so older adapters replay).
- L3 is generated one block at a time (:data:`L3_MODE` ``blockwise``): each
  topology block yields one ``l3`` pair whose prompt adds the block
  operations before it (``DONE:``) and the block to write
  (``NEXT: <path>:<block>``, :func:`block_prompt`) and whose completion is
  that block's single operation in :func:`som_core.wire.dump_block`
  form; each family adds one ``l3i`` pair that
  reads every block operation (``BLOCKS:``, :func:`imports_prompt`) and
  writes the ``CREATE_FILE`` and ``ADD_IMPORT`` operations. The corpus keeps
  those header operations before every block operation and the block
  operations in topology order, so header plus blocks is the gold list.
  ``training_summary.json`` records ``l3_mode``; a summary without it
  predates this rule (``whole``: one ``l3`` pair for the whole list).
  ``blockwise`` is retired: its replies repeated ``path`` and ``block`` and
  closed the fence with a line.
- Families are split before pairs are built, so no family's pairs sit on
  both sides; only the training side is repeated by layer weight
  (:func:`weighted_epoch`). The validation side is the frozen ``HOLDOUT``
  list (:func:`split_holdout`), the 20 families ``layer-sft-005b`` held out,
  so eval numbers stay comparable while the corpus grows; a ``HOLDOUT`` id
  missing from the corpus is refused rather than silently shrinking it.
- ``train_fraction`` keeps a prefix of one seeded order of the ``dsl``
  training families (:func:`subset_families`); validation keeps every
  validation family, so a family-count curve moves only the count.
  ``val_layers`` limits the validation passes, and so the checkpoint choice,
  to the named layers.
- Loss is masked to the completion: the prompt tokens (system line naming the
  layer plus the input record) carry no gradient.
- ``docstrings=False`` drops function and class docstrings from every ``dsl``
  completion and every rejected attempt a retry turn shows
  (:func:`som_core.dsl.drop_docstrings`), on both sides of the split; they
  held half the ``dsl`` validation loss while no fixture reads them.
- ``order_seed`` reseeds only the batch order (``iterate_batches``); the
  split, the family subset, and every pair builder keep ``seed``, so two runs
  that differ in it see the same pairs and measure run-to-run variance.
- Validation loss is reported per layer at step 0, at every ``eval_every``
  steps, and at the last step, into ``training_summary.json`` next to the
  saved adapter.
- The saved adapter is the checkpoint with the lowest mean per-layer
  validation loss among those passes (``best_step``), not the last step;
  ``val_fell`` compares that checkpoint with step 0. It is rewritten at
  each improvement, and ``adapter_config.json`` is written before step 1,
  so a run the host kills leaves its best checkpoint so far loadable.
- MLX's buffer cache is capped at ``CACHE_LIMIT_BYTES``, and every
  ``MEMORY_EVERY`` steps the batch's padded length and MLX's active, cache,
  and peak bytes are appended to ``memory.jsonl`` beside the adapter, so a
  run the host kills for memory pressure leaves its trajectory behind.
"""

from __future__ import annotations

import json
import math
import random
import time
from pathlib import Path
from typing import Any

from .dataset import load_corpora, resolve_layer_weights, split_dataset, weighted_epoch
from .paths import ROOT, sha256, write_json
from .wire import dump_block, dump_ops

LOCK = ROOT / "sources.lock.json"
LAYERS = ("l1", "l2", "l3")
PAIR_LAYERS = ("l1", "l2", "l3", "l3i")
SYSTEM = {
    "l1": "SOM L1 Planner: given a caption, emit the L1 plan as JSON.",
    "l2": "SOM L2 Topology: given an L1 plan, emit the file and block topology as JSON.",
    "l3": "SOM L3 Optimizer: given a topology, emit the operation list as JSON.",
}
L3_FORMAT = "fenced"
SYSTEM_L3_FENCED = (
    "SOM L3 Optimizer: given a topology, emit the operation list: one JSON operation per line, "
    "each INSERT_BLOCK followed by its source in a ```python fence."
)
L3_MODE = "blockwise-open"
SYSTEM_L3_BLOCK = (
    "SOM L3 Optimizer: given a topology, the block operations done so far, and the NEXT block, emit that block's "
    "one operation: a JSON line without path or block; an INSERT_BLOCK continues with a ```python line and its "
    "source to the end of the reply."
)
SYSTEM_L3I = (
    "SOM L3 Imports: given a topology and every block operation, emit each file's CREATE_FILE and ADD_IMPORT "
    "operations, one JSON operation per line."
)
HEADER_OPS = ("CREATE_FILE", "ADD_IMPORT")
HOLDOUT = frozenset(
    "som:python:" + name
    for name in (
        "01-fastapi-query-pagination", "09-pydantic-field-cross-validation", "10-security-timing-constant-auth",
        "14-pydantic-field-validator-regex", "29-stdlib-os-atomic-file-write", "31-syntax-mutable-defaults",
        "40-redis-pubsub-pipeline", "41-pandas-settingwithcopy-chained", "43-pandas-apply-vectorization-pitfall",
        "48-sklearn-imbalanced-metric-target-encoding", "54-pyyaml-safe-load-execution",
        "55-pyyaml-custom-safe-loader", "62-aiohttp-timeout-disconnects", "65-lxml-xxe-injection",
        "72-crypto-pbkdf2-iteration-count", "73-crypto-aes-cipher-mode", "77-jwt-expiration-validation",
        "78-jwt-sensitive-data-leakage", "94-anthropic-stream-lifecycle", "99-langgraph-cycle-recursion-control",
    )
)
CACHE_LIMIT_BYTES = 8 * 2**30
MEMORY_EVERY = 10


class TrainError(Exception):
    """A refusal the CLI prints as ``refused: ...``."""


def split_holdout(rows: list[dict[str, Any]], holdout: frozenset[str] = HOLDOUT) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    """Return (train, val) with exactly the ``holdout`` families on the val side."""
    missing = holdout - {r["id"] for r in rows}
    if missing:
        raise TrainError(f"holdout families missing from the corpus: {sorted(missing)}")
    return [r for r in rows if r["id"] not in holdout], [r for r in rows if r["id"] in holdout]


def subset_families(rows: list[dict[str, Any]], fraction: float, seed: int = 42) -> list[dict[str, Any]]:
    """Keep ``fraction`` of the training families, a prefix of one seeded order, so a smaller
    fraction's families are always inside a larger one's and a family-count curve varies
    only the count."""
    if not 0.0 < fraction <= 1.0:
        raise TrainError(f"train fraction {fraction} is not in (0, 1]")
    order = sorted(rows, key=lambda r: r["id"])
    random.Random(seed).shuffle(order)
    return order[: max(1, round(len(order) * fraction))]


def _compact(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"))


CONDITIONING = "cumulative"


def layer_prompt(layer: str, caption: str, plan: Any = None, topology: Any = None, conditioning: str = CONDITIONING) -> str:
    """The user turn for ``layer``: the caption plus every upstream record (or, ``previous``, the record above)."""
    if conditioning == "previous":
        return {"l1": caption, "l2": _compact(plan), "l3": _compact(topology)}[layer]
    if conditioning != CONDITIONING:
        raise TrainError(f"unknown conditioning {conditioning!r}")
    parts = [f"CAPTION:\n{caption}"]
    if layer in ("l2", "l3"):
        parts.append(f"L1:\n{_compact(plan)}")
    if layer == "l3":
        parts.append(f"L2:\n{_compact(topology)}")
    return "\n\n".join(parts)


def split_ops(ops: list[dict[str, Any]]) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    """``(header, blocks)``: the ``CREATE_FILE``/``ADD_IMPORT`` operations and the block operations, order kept."""
    return [o for o in ops if o.get("op") in HEADER_OPS], [o for o in ops if o.get("op") not in HEADER_OPS]


def block_prompt(caption: str, plan: Any, topology: Any, done: list[dict[str, Any]], path: str, block: str) -> str:
    return f"{layer_prompt('l3', caption, plan, topology)}\n\nDONE:\n{dump_ops(done) or '(none)'}\n\nNEXT: {path}:{block}"


def imports_prompt(caption: str, plan: Any, topology: Any, blocks: list[dict[str, Any]]) -> str:
    return f"{layer_prompt('l3', caption, plan, topology)}\n\nBLOCKS:\n{dump_ops(blocks)}"


def _blockwise_pairs(caption: str, plan: Any, topology: Any, ops: list[dict[str, Any]]) -> list[tuple[str, str, str]]:
    header, blocks = split_ops(ops)
    pairs = [
        ("l3", block_prompt(caption, plan, topology, blocks[:i], op["path"], op["block"]), dump_block(op))
        for i, op in enumerate(blocks)
    ]
    return pairs + [("l3i", imports_prompt(caption, plan, topology, blocks), dump_ops(header))]


def build_pairs(
    row: dict[str, Any], conditioning: str = CONDITIONING, l3_format: str = L3_FORMAT, l3_mode: str = L3_MODE
) -> list[tuple[str, str, str]]:
    """``(layer, prompt, completion)`` for every layer the row has records for."""
    meta = row.get("metadata") or {}
    decompiled = meta.get("decompiled") or {}
    caption, plan = meta.get("caption"), meta.get("plan")
    topology, ops = decompiled.get("topology"), decompiled.get("ops")
    if caption and not isinstance(caption, str):
        caption = _compact(caption)
    need = {"l1": (caption, plan), "l2": (caption, plan, topology), "l3": (caption, plan, topology, ops)}
    if conditioning == "previous":
        need = {"l1": (caption, plan), "l2": (plan, topology), "l3": (topology, ops)}
    completion = {"l1": plan, "l2": topology, "l3": ops}
    if l3_format not in ("json", L3_FORMAT):
        raise TrainError(f"unknown l3_format {l3_format!r}")
    if l3_mode not in ("whole", L3_MODE):
        raise TrainError(f"unknown l3_mode {l3_mode!r}")
    blockwise = l3_mode == L3_MODE
    if blockwise and (l3_format != L3_FORMAT or conditioning != CONDITIONING):
        raise TrainError(f"l3_mode {L3_MODE!r} needs l3_format {L3_FORMAT!r} and conditioning {CONDITIONING!r}")
    dump = {"l1": _compact, "l2": _compact, "l3": dump_ops if l3_format == L3_FORMAT else _compact}
    pairs = [
        (layer, layer_prompt(layer, caption, plan, topology, conditioning), dump[layer](completion[layer]))
        for layer in (LAYERS[:2] if blockwise else LAYERS)
        if all(need[layer])
    ]
    if blockwise and all(need["l3"]):
        pairs += _blockwise_pairs(caption, plan, topology, ops)
    return pairs


def system_line(layer: str, l3_format: str = L3_FORMAT, l3_mode: str = L3_MODE) -> str:
    if layer == "l3i":
        return SYSTEM_L3I
    if layer == "l3" and l3_mode == L3_MODE:
        return SYSTEM_L3_BLOCK
    if layer == "l3" and l3_format == L3_FORMAT:
        return SYSTEM_L3_FENCED
    return SYSTEM[layer]


def messages(
    layer: str, prompt: str, completion: str, l3_format: str = L3_FORMAT, l3_mode: str = L3_MODE
) -> list[dict[str, str]]:
    system = system_line(layer, l3_format, l3_mode)
    return [
        {"role": "system", "content": system},
        {"role": "user", "content": prompt},
        {"role": "assistant", "content": completion},
    ]


def write_lock(model: str, path: Path = LOCK) -> dict[str, Any]:
    """Resolve ``model``'s current revision, download it, and lock every file's digest."""
    from huggingface_hub import HfApi, snapshot_download

    revision = HfApi().model_info(model).sha
    local = Path(snapshot_download(model, revision=revision))
    files = {
        str(p.relative_to(local)): sha256(p)
        for p in sorted(local.rglob("*"))
        if p.is_file() and not p.name.startswith(".")
    }
    lock = {"model": model, "revision": revision, "files": files}
    write_json(path, lock)
    return lock


def fetch_locked(path: Path = LOCK) -> tuple[Path, dict[str, Any]]:
    """Download the locked snapshot and refuse any file whose digest differs."""
    if not path.exists():
        raise TrainError(f"no {path.name}; run `som lock <model>` first")
    lock = json.loads(path.read_text())
    from huggingface_hub import snapshot_download

    local = Path(snapshot_download(lock["model"], revision=lock["revision"]))
    for name, digest in lock["files"].items():
        got = sha256(local / name)
        if got != digest:
            raise TrainError(f"{lock['model']}@{lock['revision']}: {name} sha256 {got or 'missing'} != locked {digest}")
    return local, lock


def _tokenize(tokenizer, layer: str, prompt: str, completion: str) -> tuple[list[int], int]:
    from .generate import dsl_messages, revision_messages

    if layer in ("dsl_fix", "dsl_retry", "dsl_fix_retry"):
        msgs = revision_messages(*json.loads(prompt), completion)
    else:
        msgs = dsl_messages(prompt, completion) if layer in ("dsl", "dsl_swap") else messages(layer, prompt, completion)
    tokens = tokenizer.apply_chat_template(msgs, return_dict=False)
    offset = len(tokenizer.apply_chat_template(msgs[:-1], add_generation_prompt=True, return_dict=False))
    return list(tokens), offset


def _undocumented(pair: tuple[str, str, str]) -> tuple[str, str, str]:
    """A ``dsl`` pair without docstrings in its completion or its retry turn's rejected attempt."""
    from .dsl import drop_docstrings

    layer, prompt, completion = pair
    if layer in ("dsl_fix", "dsl_retry", "dsl_fix_retry"):
        som_prompt, attempt, feedback = json.loads(prompt)
        prompt = json.dumps([som_prompt, drop_docstrings(attempt), feedback])
    return layer, prompt, drop_docstrings(completion)


def _stop_router_gradients() -> None:
    """Qwen3-MoE routes with ``take_along_axis(gates, argpartition(gates))``;
    mlx-lm 0.31.3 leaves the expert indices differentiable, so the first
    backward pass dies in ``gather_axis``. The indices get ``stop_gradient``;
    the scores still carry the router's gradient."""
    import mlx.core as mx
    from mlx_lm.models import qwen3_moe

    def call(self, x):
        gates = mx.softmax(self.gate(x), axis=-1, precise=True)
        inds = mx.stop_gradient(mx.argpartition(gates, kth=-self.top_k, axis=-1)[..., -self.top_k:])
        scores = mx.take_along_axis(gates, inds, axis=-1)
        if self.norm_topk_prob:
            scores /= mx.sum(scores, axis=-1, keepdims=True)
        return (self.switch_mlp(x, inds) * scores[..., None]).sum(axis=-2)

    qwen3_moe.Qwen3MoeSparseMoeBlock.__call__ = call


def train(
    out: Path,
    data_dirs: list[Path] | None = None,
    layer_weights: list[str] | None = None,
    iters: int = 600,
    batch_size: int = 1,
    learning_rate: float = 1e-4,
    lora_layers: int = 16,
    rank: int = 16,
    eval_every: int = 100,
    max_seq_length: int = 4096,
    val_ratio: float = 0.2,
    seed: int = 42,
    holdout: bool = True,
    lock_path: Path = LOCK,
    task: str = "layers",
    prompts_dir: Path | None = None,
    dropout: bool = True,
    dropout_copies: int = 1,
    revisions: bool = False,
    retries: bool = False,
    swaps: bool = False,
    fix_retries: bool = False,
    train_fraction: float = 1.0,
    val_layers: tuple[str, ...] | None = None,
    docstrings: bool = True,
    order_seed: int | None = None,
    log=print,
) -> dict[str, Any]:
    """``task="layers"`` trains the L1/L2/L3 records; ``task="dsl"`` trains
    SOM prompt -> DSL (``som_core.dsl_pairs``) and validates on ``val_ratio``
    of the non-holdout families, so the holdout never picks the checkpoint."""
    import mlx.core as mx
    import mlx.nn as nn
    import mlx.optimizers as optim
    import numpy as np
    from mlx.utils import tree_flatten
    from mlx_lm import load
    from mlx_lm.tuner.trainer import default_loss, iterate_batches
    from mlx_lm.tuner.utils import linear_to_lora_layers

    # A long, variable-length pair set grows MLX's buffer cache without bound;
    # capping it kept 500-step runs under the host's memory-pressure reaper.
    mx.set_cache_limit(CACHE_LIMIT_BYTES)
    _stop_router_gradients()
    model_dir, lock = fetch_locked(lock_path)
    rows = load_corpora(data_dirs)
    train_rows, val_rows = split_holdout(rows) if holdout else split_dataset(rows, val_ratio=val_ratio, seed=seed)
    if task == "dsl":
        from .dataset import find_default_corpora
        from .dsl_pairs import dsl_pairs, fix_pairs, fix_retry_pairs, retry_pairs, swap_pairs

        train_rows, val_rows = split_dataset(train_rows, val_ratio=val_ratio, seed=seed)
        if train_fraction < 1.0:
            train_rows = subset_families(train_rows, train_fraction, seed=seed)
        corpora =[Path(d) for d in data_dirs] if data_dirs else find_default_corpora()

        built: dict[str, list[tuple[str, str, str]]] = {}  # weighted epochs repeat rows; retry pairs compile each attempt

        def pairs(r: dict[str, Any]) -> list[tuple[str, str, str]]:
            if r["id"] not in built:
                corpus = next(c for c in corpora if (c / "families" / r["family"]).is_dir())
                built[r["id"]] = (dsl_pairs(r, corpus, prompts_dir, dropout=dropout, seed=seed, dropout_copies=dropout_copies)
                                  + (fix_pairs(r, corpus, prompts_dir) if revisions else [])
                                  + (retry_pairs(r, corpus) if retries else [])
                                  + (swap_pairs(r, corpus, prompts_dir, seed=seed) if swaps else [])
                                  + (fix_retry_pairs(r, corpus, seed=seed) if fix_retries else []))
                if not docstrings:
                    built[r["id"]] = [_undocumented(p) for p in built[r["id"]]]
            return built[r["id"]]
    elif task == "layers":
        pairs = build_pairs
    else:
        raise TrainError(f"unknown task {task!r}")
    epoch_rows = weighted_epoch(train_rows, resolve_layer_weights(layer_weights), seed=seed)

    model, tokenizer = load(str(model_dir))
    train_set = [_tokenize(tokenizer, *p) for r in epoch_rows for p in pairs(r)]
    val_sets: dict[str, list[tuple[list[int], int]]] = {
        layer: [] for layer in (("dsl", *(["dsl_fix"] if revisions else []), *(["dsl_retry"] if retries else []), *(["dsl_swap"] if swaps else []), *(["dsl_fix_retry"] if fix_retries else [])) if task == "dsl" else PAIR_LAYERS)
    }
    if val_layers:
        if unknown := set(val_layers) - set(val_sets):
            raise TrainError(f"val layers {sorted(unknown)} are not trained; trained: {sorted(val_sets)}")
        val_sets = {layer: data for layer, data in val_sets.items() if layer in val_layers}
    for r in val_rows:
        for p in pairs(r):
            if p[0] in val_sets:
                val_sets[p[0]].append(_tokenize(tokenizer, *p))
    if not train_set or not all(val_sets.values()):
        raise TrainError(f"empty pair set: train {len(train_set)}, val {({k: len(v) for k, v in val_sets.items()})}")
    longest = max(len(t) for t, _ in train_set + [x for v in val_sets.values() for x in v])
    log(
        f"model {lock['model']}@{lock['revision'][:12]}  families train {len(train_rows)} val {len(val_rows)}  "
        f"pairs train {len(train_set)} val {({k: len(v) for k, v in val_sets.items()})}  longest {longest} tokens"
    )

    model.freeze()
    linear_to_lora_layers(model, lora_layers, {"rank": rank, "scale": 20.0, "dropout": 0.0})
    trainable = sum(v.size for _, v in tree_flatten(model.trainable_parameters()))
    log(f"trainable parameters: {trainable}")
    optimizer = optim.Adam(learning_rate=learning_rate)
    loss_and_grad = nn.value_and_grad(model, default_loss)

    def evaluate() -> dict[str, float]:
        model.eval()
        result = {}
        for layer, data in val_sets.items():
            total = ntok = 0.0
            for batch, lengths in iterate_batches(data, 1, max_seq_length):
                loss, toks = default_loss(model, batch, lengths)
                mx.eval(loss, toks)
                total += loss.item() * toks.item()
                ntok += toks.item()
            result[layer] = total / ntok
        model.train()
        return result

    def mean(v: dict[str, float]) -> float:
        return sum(v.values()) / len(v)

    history = []
    start = time.time()
    val = evaluate()
    history.append({"step": 0, "val": val})
    best: dict[str, Any] = {"step": 0, "val": val, "weights": None}
    log(f"step 0  val {_fmt(val)}")
    order = seed if order_seed is None else order_seed
    np.random.seed(order)
    batches = iterate_batches(train_set, batch_size, max_seq_length, loop=True, seed=order)
    window: list[float] = []
    out.mkdir(parents=True, exist_ok=True)
    write_json(
        out / "adapter_config.json",
        {"fine_tune_type": "lora", "num_layers": lora_layers, "lora_parameters": {"rank": rank, "scale": 20.0, "dropout": 0.0}},
    )
    memory_log = (out / "memory.jsonl").open("w")
    for step in range(1, iters + 1):
        batch, lengths = next(batches)
        (loss, _), grads = loss_and_grad(model, batch, lengths)
        optimizer.update(model, grads)
        mx.eval(model.parameters(), optimizer.state, loss)
        window.append(loss.item())
        if step % MEMORY_EVERY == 0:
            memory_log.write(json.dumps({"step": step, "seq": int(batch.shape[1]), "active": mx.get_active_memory(), "cache": mx.get_cache_memory(), "peak": mx.get_peak_memory(), "t": round(time.time() - start)}) + "\n")
            memory_log.flush()
        if not math.isfinite(window[-1]):
            raise TrainError(f"step {step}: loss is {window[-1]}")
        if step % eval_every == 0 or step == iters:
            val = evaluate()
            train_loss = sum(window) / len(window)
            window = []
            history.append({"step": step, "train": train_loss, "val": val})
            improved = mean(val) < mean(best["val"])
            if improved:
                weights = dict(tree_flatten(model.trainable_parameters()))
                mx.eval(weights)
                best = {"step": step, "val": val, "weights": {k: mx.array(v) for k, v in weights.items()}}
                # a multi-hour run survives a host restart with its best checkpoint so far
                mx.save_safetensors(str(out / "adapters.safetensors"), best["weights"])
            log(f"step {step}  train {train_loss:.4f}  val {_fmt(val)}  {time.time() - start:.0f}s  peak {mx.get_peak_memory() / 2**30:.1f}G" + ("  *best" if improved else ""))

    if best["weights"] is None:
        raise TrainError(f"no checkpoint beat step 0 mean validation loss {mean(best['val']):.4f}")
    memory_log.close()
    first, last = history[0]["val"], best["val"]
    summary = {
        "model": lock["model"],
        "revision": lock["revision"],
        "lock": str(Path(lock_path).resolve().relative_to(ROOT)) if Path(lock_path).resolve().is_relative_to(ROOT) else str(lock_path),
        "lock_sha256": sha256(lock_path),
        "task": task,
        "prompts_dir": str(prompts_dir) if prompts_dir else None,
        "dropout": dropout,
        "dropout_copies": dropout_copies,
        "revisions": revisions,
        "retries": retries,
        "swaps": swaps,
        "fix_retries": fix_retries,
        "docstrings": docstrings,
        "conditioning": CONDITIONING,
        "l3_format": L3_FORMAT,
        "l3_mode": L3_MODE,
        "families": {"train": sorted(r["id"] for r in train_rows), "val": sorted(r["id"] for r in val_rows)},
        "pairs": {"train": len(train_set), "val": {k: len(v) for k, v in val_sets.items()}},
        "config": {
            "iters": iters, "batch_size": batch_size, "learning_rate": learning_rate, "lora_layers": lora_layers,
            "rank": rank, "eval_every": eval_every, "max_seq_length": max_seq_length, "val_ratio": val_ratio, "seed": seed, "order_seed": order_seed,
            "split": "holdout" if holdout else "ratio", "train_fraction": train_fraction,
            "val_layers": sorted(val_sets),
        },
        "history": history,
        "best_step": best["step"],
        "val_fell": {layer: last[layer] < first[layer] for layer in val_sets},
        "seconds": round(time.time() - start, 1),
    }
    write_json(out / "training_summary.json", summary)
    return summary


def _fmt(val: dict[str, float]) -> str:
    return "  ".join(f"{k} {v:.4f}" for k, v in val.items())
