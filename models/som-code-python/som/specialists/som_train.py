"""Isolated SOM training.  It reuses the frozen v4 Q/V LoRA architecture only."""
from __future__ import annotations

import hashlib
import importlib
import json
import os
import random
import time
import uuid
from pathlib import Path

import numpy as np

from ..paths import ROOT, read_json, sha256, write_json
from ..python_smoke import audit_python_smoke, smoke_rows as audited_python_smoke_rows
from . import som_data
from .som_config import DATA, SOURCE_LOCK, TRAIN_CONFIG, V4_ADAPTER_SHA256, corpus_path, corpus_source_lock, run_path, source_lock_sha256, verify_v4_start
from .training_balance import candidate_correctness_balance

_BOUND = ("som/specialists/som_train.py", "som/specialists/som_eval.py",
          "som/specialists/som_metrics.py", "som/specialists/som_data.py", "som/specialists/som_config.py",
          "som/specialists/som_runtime.py", "som/specialists/input.py", "som/specialists/loss.py",
          "som/specialists/training_balance.py", "som/specialists/model.py",
          "som/specialists/score_contract.py", "som/developer/modern_model.py")
# Mirrors the frozen loss module. Keeping this literal permits CPU-only
# audit tests in environments where importing MLX would initialise Metal.
_LOSS_CONFIG = {"binary_weight": .50, "present_margin_weight": .25, "review_margin_weight": .25,
                "teacher_kl_weight": .05, "margin": .50}
_MLX_GPU_MESSAGE = "The required MLX Metal GPU is unavailable; no CPU fallback is used."
_PYTHON_SMOKE_CORPUS_ROOT_ENV = "SOM_PYTHON_SMOKE_CORPUS_ROOT"


def _metal_error(error: BaseException) -> RuntimeError:
    """Convert MLX's headless Metal import failure into the SOM UX contract."""
    return RuntimeError(_MLX_GPU_MESSAGE)


def _training_runtime():
    """Load MLX only after SOM data gates pass, with a clear GPU-only failure."""
    try:
        mx = importlib.import_module("mlx.core")
        nn = importlib.import_module("mlx.nn")
        optim = importlib.import_module("mlx.optimizers")
        tree_map = importlib.import_module("mlx.utils").tree_map
    except ImportError as error:
        raise _metal_error(error) from error
    except RuntimeError as error:
        if "metal" in str(error).lower():
            raise _metal_error(error) from error
        raise
    try:
        feature_executor = importlib.import_module("som.specialists.som_runtime")
        input_runtime = importlib.import_module("som.specialists.input")
        model_runtime = importlib.import_module("som.specialists.model")
    except ImportError as error:
        raise RuntimeError("SOM internal training runtime is unavailable; Metal availability was not checked by this error.") from error
    return (mx, nn, optim, tree_map, feature_executor, input_runtime.accumulation_groups,
            input_runtime.checkpoint_boundaries, model_runtime.load_model)


def _python_smoke_corpus_root() -> Path:
    """Return the optional, independently staged Python smoke corpus root.

    This switch is deliberately private to the smoke command.  Formal Python
    training keeps using ``corpus_path("python")`` and cannot be redirected by
    this environment variable.
    """
    configured = os.environ.get(_PYTHON_SMOKE_CORPUS_ROOT_ENV)
    if configured is None:
        return corpus_path("python")
    candidate = Path(configured)
    if not candidate.is_absolute():
        raise ValueError(f"{_PYTHON_SMOKE_CORPUS_ROOT_ENV} must be an absolute directory.")
    root = candidate.resolve()
    if not root.is_dir():
        raise ValueError(f"{_PYTHON_SMOKE_CORPUS_ROOT_ENV} must name an existing directory.")
    if not (root / "smoke").is_dir():
        raise ValueError(f"{_PYTHON_SMOKE_CORPUS_ROOT_ENV} must contain a smoke directory.")
    return root


def _passed(value) -> bool:
    return isinstance(value, dict) and (value.get("status") == "passed" or value.get("passed") is True) and not value.get("errors")


def verify_pre_final(domain: str = "frontend") -> dict:
    """Validate the immutable audit certificate without opening final rows.

    Preparation creates this certificate after its full family audit.  Training
    and calibration only verify its hashes and then read their own split.
    """
    paths = som_data._paths(domain)
    manifest_path = paths["manifest"]
    report_path = paths["audit_json"]
    if not manifest_path.exists() or not report_path.exists():
        raise ValueError("SOM prepared audit certificate is missing.")
    manifest, audited = read_json(manifest_path), read_json(report_path)
    source_lock = paths["source_lock"]
    verified = {"status": "passed", "protocol": manifest.get("protocol"), "source_lock_sha256": manifest.get("source_lock_sha256")}
    if (manifest.get("protocol") != "som-v1" or not source_lock.exists()
            or manifest.get("source_lock_sha256") != sha256(source_lock)
            or not _passed(verified) or not _passed(audited)):
        raise ValueError("SOM data audit failed; smoke and training are blocked.")
    return {"verify": verified, "audit": audited}


def audit_or_raise(domain: str = "frontend") -> dict:
    """Run the full audit.  Call this only after selection is frozen."""
    verified = som_data.verify(domain)
    audited = som_data.audit(domain)
    if not _passed(verified) or not _passed(audited):
        raise ValueError("SOM data audit failed; smoke and training are blocked.")
    return {"verify": verified, "audit": audited}


def _rows(split: str, domain: str = "frontend") -> list[dict]:
    (audit_or_raise if split == "final" else verify_pre_final)(domain)
    paths = som_data._paths(domain)
    manifest = read_json(paths["manifest"])
    path = corpus_path(domain) / f"{split}-{domain}.jsonl"
    if manifest.get("files", {}).get(f"{split}-{domain}.jsonl") != sha256(path):
        raise ValueError(f"SOM {split} rows differ from the audited manifest.")
    return list(som_data.rows(split, domain))


def _row_hash(rows) -> str:
    return hashlib.sha256(json.dumps(rows, sort_keys=True, ensure_ascii=False, separators=(",", ":")).encode()).hexdigest()


def protocol(domain: str, *, train_rows: list[dict] | None = None) -> dict:
    if domain not in ("frontend", "python", "mixed"):
        raise ValueError("SOM training domain must be frontend, python, or mixed.")
    from .som_config import CONTROL_ROW_HASHES
    data = corpus_path(domain) if domain != "mixed" else DATA
    lock = corpus_source_lock(domain) if domain != "mixed" else SOURCE_LOCK
    if train_rows is None:
        train_rows, _, _ = training_rows(domain)
    return {"protocol": "som-v1", "domain": domain, "training": dict(TRAIN_CONFIG), "seed": verify_v4_start(),
            "data_manifest_sha256": sha256(data / "manifest.json"), "source_lock": str(lock),
            "source_lock_sha256": sha256(lock),
            "mixed_control_row_hashes": CONTROL_ROW_HASHES if domain == "mixed" else None,
            "candidate_correctness_balance": candidate_correctness_balance(train_rows),
            "code_sha256": {name: sha256(ROOT / name) for name in _BOUND if (ROOT / name).exists()}}


def _control_rows(domain: str, split: str) -> list[dict]:
    """Read byte-locked Python or Rust control rows."""
    from .som_config import MIXED_CONTROL
    path = MIXED_CONTROL / "rows" / f"{split}-{domain}.jsonl"
    return [json.loads(line) for line in path.read_text().splitlines() if line.strip()]


def mixed_rows(split: str) -> tuple[list[dict], dict]:
    frontend = _rows(split, "frontend")
    python, rust = _control_rows("python", split), _control_rows("rust", split)
    all_rows = frontend + python + rust
    hashes = {"frontend": _row_hash(frontend), "python": _row_hash(python), "rust": _row_hash(rust),
              "mixed_control_manifest": sha256(ROOT / "data" / "som" / "mixed-control" / "manifest.json")}
    from . import som_config
    locked = getattr(som_config, "CONTROL_ROW_HASHES", None)
    if not isinstance(locked, dict):
        raise ValueError("SOM config does not pin the required control row hashes.")
    expected = locked.get(split, {})
    for domain in ("python", "rust"):
        path = ROOT / "data" / "som" / "mixed-control" / "rows" / f"{split}-{domain}.jsonl"
        actual = sha256(path)
        hashes[domain] = actual
        if expected.get(domain) != actual:
            raise ValueError(f"Locked mixed-control {domain} {split} rows differ; mixed training is blocked.")
    return all_rows, hashes


def training_rows(domain: str) -> tuple[list[dict], list[dict], dict]:
    if domain == "frontend":
        train, validation = _rows("train"), _rows("validation")
        return train, validation, {"frontend_train": _row_hash(train), "frontend_validation": _row_hash(validation)}
    if domain == "python":
        train, validation = _rows("train", "python"), _rows("validation", "python")
        return train, validation, {"python_train": _row_hash(train), "python_validation": _row_hash(validation)}
    if domain == "mixed":
        train, train_hashes = mixed_rows("train")
        validation, validation_hashes = mixed_rows("validation")
        return train, validation, {"train": train_hashes, "validation": validation_hashes}
    raise ValueError("SOM training domain must be frontend, python, or mixed.")


def loss_component_accounting(scores, gold_index: int, missing_correct_patch: bool, prior=None, candidate_positive_weight: float = 1.0) -> dict:
    """Return the frozen objective as named scalar terms for run logs.

    This numpy version also gives CPU tests a way to check the accounting.
    """
    scores = np.asarray(scores, dtype=float)
    review = np.array([0.0])
    logits = np.concatenate([scores, review])
    gold = len(scores) if missing_correct_patch else gold_index
    maximum = logits.max()
    ranking = float(maximum + np.log(np.exp(logits - maximum).sum()) - logits[gold])
    truth = np.array([float((not missing_correct_patch) and i == gold_index) for i in range(len(scores))])
    if candidate_positive_weight <= 0:
        raise ValueError("Candidate correctness positive weight must be positive.")
    binary_terms = np.logaddexp(0., scores) - truth * scores
    binary_weights = 1. + truth * (candidate_positive_weight - 1.)
    binary = float(np.sum(binary_weights * binary_terms) / np.sum(binary_weights))
    if missing_correct_patch:
        margin = float(np.mean(np.maximum(0., scores + _LOSS_CONFIG["margin"])))
        present_margin, review_margin = 0.0, _LOSS_CONFIG["review_margin_weight"] * margin
    else:
        others = np.concatenate([scores[:gold_index], scores[gold_index + 1:], review])
        margin = float(np.mean(np.maximum(0., _LOSS_CONFIG["margin"] - scores[gold_index] + others)))
        present_margin, review_margin = _LOSS_CONFIG["present_margin_weight"] * margin, 0.0
    teacher = 0.0
    if prior is not None:
        prior = np.asarray(prior, dtype=float)
        target = 1 / (1 + np.exp(-prior))
        log_yes = lambda value: -np.logaddexp(0., -value)
        log_no = lambda value: -np.logaddexp(0., value)
        teacher = float(np.mean(target * (log_yes(prior) - log_yes(scores)) + (1 - target) * (log_no(prior) - log_no(scores))))
    weighted_binary = _LOSS_CONFIG["binary_weight"] * binary
    weighted_teacher = _LOSS_CONFIG["teacher_kl_weight"] * teacher
    return {"ranking": ranking, "binary": binary, "weighted_binary": weighted_binary, "present_margin": present_margin,
            "review_margin": review_margin, "teacher_kl": teacher, "weighted_teacher_kl": weighted_teacher,
            "total": ranking + weighted_binary + present_margin + review_margin + weighted_teacher}


def _ordered(rows):
    result = list(rows)
    random.Random(4701).shuffle(result)
    return result


def checkpoint_groups(start: int, total: int, accumulation: int, boundaries=(300, 600, 900, 1200)):
    """Accumulate normally, but end a smaller group at each frozen checkpoint."""
    if not 0 <= start <= total or accumulation < 1:
        raise ValueError("Invalid SOM checkpoint grouping inputs.")
    cursor = start
    allowed = tuple(boundary for boundary in boundaries if start < boundary <= total)
    while cursor < total:
        next_boundary = next((boundary for boundary in allowed if boundary > cursor), total)
        end = min(cursor + accumulation, next_boundary, total)
        yield range(cursor, end)
        cursor = end


def checkpoint_marks(total: int) -> dict[int, int]:
    """Return the exact frozen 0/25/50/75/100 percent sample boundaries."""
    if total < 4 or total % 4:
        raise ValueError("SOM training rows must divide evenly into four checkpoint fractions.")
    return {total * percent // 100: percent for percent in (25, 50, 75, 100)}


def _save_adapter(model, destination: Path, *, parent: str, frozen: dict):
    import mlx.core as mx
    from mlx.utils import tree_flatten
    destination.mkdir(parents=True, exist_ok=True)
    mx.save_safetensors(str(destination / "adapter.safetensors"), dict(tree_flatten(model.trainable_parameters())))
    write_json(destination / "model_config.json", {"protocol": "som-v1", "parent_adapter_sha256": parent,
                                                     "training_protocol_sha256": hashlib.sha256(json.dumps(frozen, sort_keys=True).encode()).hexdigest()})


def _restore_adapter(model, checkpoint: Path, frozen: dict):
    import mlx.core as mx
    from mlx.utils import tree_flatten
    config = read_json(checkpoint / "model_config.json")
    expected_protocol = hashlib.sha256(json.dumps(frozen, sort_keys=True).encode()).hexdigest()
    if config.get("protocol") != "som-v1" or config.get("parent_adapter_sha256") != V4_ADAPTER_SHA256 or config.get("training_protocol_sha256") != expected_protocol:
        raise ValueError("SOM checkpoint provenance differs from the frozen protocol.")
    weights, expected = mx.load(str(checkpoint / "adapter.safetensors")), dict(tree_flatten(model.trainable_parameters()))
    if weights.keys() != expected.keys() or any(weights[key].shape != expected[key].shape for key in expected):
        raise ValueError("SOM checkpoint parameter shapes differ.")
    model.load_weights(list(weights.items()), strict=False)
    mx.eval(model.parameters())


def load_som_model(checkpoint: Path, frozen: dict | None = None):
    """Load a SOM adapter through SOM provenance checks."""
    from .model import load_model
    checkpoint = Path(checkpoint)
    if frozen is None:
        run = checkpoint.parent.parent
        frozen = read_json(run / "training-protocol.json")
    model, tokenizer = load_model()
    _restore_adapter(model, checkpoint, frozen)
    model.eval()
    return model, tokenizer


def _smoke_rows(domain: str, *, python_corpus_root: Path | None = None) -> list[dict]:
    if domain == "python":
        root = corpus_path("python") if python_corpus_root is None else python_corpus_root
        rows, _ = audited_python_smoke_rows(root)
        return rows
    return _ordered(_rows("train", domain))[:int(TRAIN_CONFIG["smoke_rows"])]


def _frozen_hash(value: dict) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def _smoke_provenance(domain: str = "frontend", *, python_corpus_root: Path | None = None) -> dict:
    if domain == "python":
        # Do not implicitly read the staging environment here.  Formal
        # training calls this helper through _require_smoke and must retain the
        # repository corpus root.  smoke() passes its validated staging root.
        root = corpus_path("python") if python_corpus_root is None else python_corpus_root
        audit = audit_python_smoke(root)
        return {"protocol": "som-v1", "domain": "python", "corpus_kind": "smoke",
                "smoke_manifest_sha256": audit["smoke_manifest_sha256"],
                "source_lock_sha256": audit["source_lock_sha256"],
                "family_ledger_sha256": audit["family_ledger_sha256"],
                "row_sha256": audit["row_sha256"], "parent_v4": verify_v4_start()}
    data, lock = corpus_path(domain), corpus_source_lock(domain)
    return {"protocol": "som-v1", "domain": domain, "data_manifest_sha256": sha256(data / "manifest.json"),
            "source_lock_sha256": sha256(lock), "parent_v4": verify_v4_start(),
            "row_sha256": _row_hash(_smoke_rows(domain))}


def _require_smoke(path: Path, domain: str = "frontend") -> dict:
    if not path.exists():
        raise ValueError(f"A passed provenance-matching {domain} SOM smoke result is required before formal training.")
    result = read_json(path)
    if not result.get("passed") or result.get("protocol") != "som-v1" or result.get("provenance") != _smoke_provenance(domain):
        raise ValueError(f"{domain.capitalize()} SOM smoke result is missing, failed, or differs from current provenance.")
    return result


def _smoke_protocol(domain: str, audit: dict | None = None, *, python_corpus_root: Path | None = None,
                    smoke_rows: list[dict] | None = None) -> dict:
    """Freeze smoke inputs without loading or requiring the formal corpus."""
    if domain != "python":
        return protocol(domain, train_rows=smoke_rows)
    root = corpus_path("python") if python_corpus_root is None else python_corpus_root
    audit = audit or audit_python_smoke(root)
    fixed_rows = _smoke_rows("python", python_corpus_root=root) if smoke_rows is None else smoke_rows
    return {"protocol": "som-v1", "domain": "python", "corpus_kind": "smoke",
            "training": dict(TRAIN_CONFIG), "seed": verify_v4_start(),
            "smoke_manifest_sha256": audit["smoke_manifest_sha256"],
            "source_lock_sha256": audit["source_lock_sha256"],
            "family_ledger_sha256": audit["family_ledger_sha256"],
            "row_sha256": audit["row_sha256"],
            "candidate_correctness_balance": candidate_correctness_balance(fixed_rows),
            "code_sha256": {name: sha256(ROOT / name) for name in _BOUND if (ROOT / name).exists()}}


def _smoke_snapshot(domain: str, rows: int, before: dict, after: dict,
                    *, passes: int, updates: int, reduction: float,
                    history: list[dict], passed: bool = False,
                    status: str = "running", audit: dict | None = None,
                    row_sha256: str | None = None,
                    provenance: dict | None = None) -> dict:
    """Build one deterministic, inspectable smoke result snapshot."""
    return {
        "protocol": "som-v1", "domain": domain, "status": status,
        "rows": rows, "before": before, "after": after,
        "loss_reduction": float(reduction), "passes": int(passes),
        "updates": int(updates), "pass_history": list(history),
        "passed": bool(passed), "audit": audit, "row_sha256": row_sha256,
        "provenance": provenance,
    }


def _resume_state(target: Path, frozen: dict, hashes: dict) -> tuple[Path, dict]:
    """Validate an interrupted run before MLX loads or any checkpoint changes."""
    if not (target / "training-protocol.json").exists() or read_json(target / "training-protocol.json") != frozen:
        raise ValueError("Frozen SOM training protocol changed; resume is blocked.")
    summary_path, latest_path = target / "training_summary.json", target / "latest.json"
    if not summary_path.exists() or not latest_path.exists():
        raise ValueError("Interrupted SOM training state is incomplete; resume is blocked.")
    state, latest = read_json(summary_path), read_json(latest_path)
    if state.get("status") != "interrupted" or state.get("protocol") != "som-v1" or state.get("frozen_protocol_sha256") != _frozen_hash(frozen):
        raise ValueError("Only a valid interrupted SOM run can resume.")
    if state.get("row_hashes") != hashes or not isinstance(latest.get("checkpoint"), str):
        raise ValueError("SOM resume row provenance differs from this run.")
    expected_context = hashlib.sha256(json.dumps({"protocol": frozen, "rows": hashes}, sort_keys=True).encode()).hexdigest()
    if state.get("context_sha256") != expected_context:
        raise ValueError("SOM resume cache context differs from this run.")
    relative = Path(latest["checkpoint"])
    if relative.is_absolute() or ".." in relative.parts:
        raise ValueError("SOM resume checkpoint path is invalid.")
    checkpoint = target / relative
    if str(relative) not in state.get("checkpoints", ()) or not checkpoint.exists():
        raise ValueError("SOM resume checkpoint is not part of the saved run.")
    for name in ("adapter.safetensors", "model_config.json", "optimizer.npz", "rng.npz", "trainer.json"):
        if not (checkpoint / name).exists():
            raise ValueError(f"SOM resume checkpoint is missing {name}.")
    trainer = read_json(checkpoint / "trainer.json")
    for key in ("samples_seen", "updates", "row_hashes", "feature_hashes", "frozen_protocol_sha256", "context_sha256"):
        if trainer.get(key) != state.get(key):
            raise ValueError("SOM resume trainer state differs from the run summary.")
    for split, expected in state.get("feature_hashes", {}).items():
        index = target / "features" / split / "index.json"
        if not index.exists() or sha256(index) != expected:
            raise ValueError("SOM frozen feature cache changed; resume is blocked.")
    return checkpoint, state


def smoke(domain: str = "frontend", run=None) -> dict:
    """Run the required 32-row audit-gated smoke test without formal training."""
    if domain not in ("frontend", "python"):
        raise ValueError("SOM smoke supports frontend or python only.")
    smoke_audit = None
    python_smoke_root = None
    if domain == "python":
        # This is intentionally not ``require_ready('python')``.  The smoke
        # contract is a different, audited 32-row corpus from formal data.
        python_smoke_root = _python_smoke_corpus_root()
        _, smoke_audit = audited_python_smoke_rows(python_smoke_root)
    else:
        verify_pre_final(domain)
    # Import MLX training only after the no-GPU audit path succeeds.  The
    # frozen feature executor is used only for fixed Q/V-LoRA feature mechanics;
    # all input rows and cache provenance below are SOM.
    mx, nn, optim, tree_map, legacy, accumulation_groups, _, load_model = _training_runtime()
    target = Path(run or run_path(domain)); target.mkdir(parents=True, exist_ok=True)
    rows = _smoke_rows(domain, python_corpus_root=python_smoke_root)
    started = time.monotonic()
    model, tokenizer = load_model(start_from_v4=True)
    frozen = _smoke_protocol(domain, smoke_audit, python_corpus_root=python_smoke_root, smoke_rows=rows)
    context = hashlib.sha256(json.dumps({"protocol": frozen, "smoke_rows": _row_hash(rows)}, sort_keys=True).encode()).hexdigest()
    cache = target / "smoke-features" / context[:16]
    stop = lambda: time.monotonic() - started >= TRAIN_CONFIG["max_seconds"]
    if legacy._cache_features(model, tokenizer, rows, cache, context, stop) is None:
        raise RuntimeError("SOM smoke preparation exceeded the two-hour limit.")
    before = legacy._metrics(model, tokenizer, rows, cache)
    optimizer = optim.AdamW(learning_rate=TRAIN_CONFIG["learning_rate"], weight_decay=0)
    optimizer.init(model.trainable_parameters())
    gradient = nn.value_and_grad(model, legacy._loss)
    updates, after, reduction = 0, before, 0.0
    history = []
    result_path = target / "smoke.json"
    # Keep one result path for the entire bounded run.  The feature directory
    # above remains the same locked cache for every pass.
    write_json(result_path, _smoke_snapshot(
        domain, len(rows), before, before, passes=0, updates=0,
        reduction=0.0, history=history,
    ))
    for passes in range(1, 21):
        for group in accumulation_groups(0, len(rows), TRAIN_CONFIG["gradient_accumulation"]):
            if stop():
                raise RuntimeError("SOM smoke exceeded the two-hour limit.")
            accumulated = None
            for index in group:
                value, grads = gradient(model, legacy._load_feature(cache, index))
                mx.eval(value, grads)
                accumulated = grads if accumulated is None else tree_map(lambda a, b: a + b, accumulated, grads)
            optimizer.update(model, tree_map(lambda value: value / len(group), accumulated))
            mx.eval(model.parameters(), optimizer.state)
            updates += 1
        after = legacy._metrics(model, tokenizer, rows, cache)
        reduction = (before["loss"] - after["loss"]) / before["loss"] if before["loss"] else 0.0
        history.append({"pass": passes, "updates": updates, "metrics": after,
                        "loss_reduction": reduction})
        write_json(result_path, _smoke_snapshot(
            domain, len(rows), before, after, passes=passes, updates=updates,
            reduction=reduction, history=history,
        ))
        if after["accuracy"] >= TRAIN_CONFIG["smoke_min_accuracy"] and reduction >= TRAIN_CONFIG["smoke_min_loss_reduction"]:
            break
    result = _smoke_snapshot(
        domain, len(rows), before, after, passes=passes, updates=updates,
        reduction=reduction, history=history,
        passed=after["accuracy"] >= TRAIN_CONFIG["smoke_min_accuracy"] and reduction >= TRAIN_CONFIG["smoke_min_loss_reduction"],
        status="completed", audit=smoke_audit if domain == "python" else verify_pre_final(domain),
        row_sha256=_row_hash(rows), provenance=_smoke_provenance(domain, python_corpus_root=python_smoke_root),
    )
    write_json(result_path, result)
    return result


def train(domain: str = "frontend", run=None, *, resume: bool = False) -> dict:
    """Prepare an audit-gated SOM run.  Formal optimizer execution is explicit.

    The wrapper intentionally refuses to use a mutable historical run protocol. The
    active optimizer implementation is supplied by ``run_training`` below.
    """
    if domain not in ("frontend", "python", "mixed"):
        raise ValueError("SOM training domain must be frontend, python, or mixed.")
    if domain == "python":
        # Formal training requires both contracts.  The formal audit runs
        # before any model import; its own fixed oracle never sees requests.
        from .som_preflight import require_ready
        require_ready("python")
        audit = audit_or_raise("python")
    else:
        audit = verify_pre_final("frontend" if domain == "mixed" else domain)
    target = Path(run or run_path(domain))
    target.mkdir(parents=True, exist_ok=True)
    smoke_target = target if domain in ("frontend", "python") else run_path("frontend")
    if domain == "frontend" and not (smoke_target / "smoke.json").exists():
        smoke(domain, target)
    if domain in ("frontend", "python", "mixed"):
        smoke_path = target / "smoke.json"
        if domain == "mixed": smoke_path = smoke_target / "smoke.json"
        _require_smoke(smoke_path, "frontend" if domain == "mixed" else domain)
    train_rows, validation_rows, hashes = training_rows(domain)
    frozen = protocol(domain, train_rows=train_rows)
    protocol_path = target / "training-protocol.json"
    if resume:
        if not protocol_path.exists() or read_json(protocol_path) != frozen:
            raise ValueError("Frozen SOM training protocol changed; resume is blocked.")
    else:
        if protocol_path.exists() or (target / "training_summary.json").exists() or (target / "latest.json").exists():
            raise ValueError("SOM run already exists; use --resume only for a valid interrupted run.")
        write_json(protocol_path, frozen)
    return run_training(domain, target, train_rows, validation_rows, hashes, frozen, audit, resume=resume)


def run_training(domain: str, target: Path, train_rows: list[dict], validation_rows: list[dict], hashes: dict, frozen: dict, audit: dict, *, resume: bool) -> dict:
    """Train with the frozen Q/V LoRA implementation.

    Reuse is intentionally narrow: the frozen implementation supplies the tested 4-bit model and
    objective, while all rows, paths, provenance and checkpoints are SOM.
    """
    if resume:
        resume_checkpoint, state = _resume_state(target, frozen, hashes)
    elif (target / "training_summary.json").exists():
        raise ValueError("SOM run already exists; use --resume only for a valid interrupted run.")
    else:
        resume_checkpoint, state = None, None
    mx, nn, optim, tree_map, feature_executor, accumulation_groups, checkpoint_boundaries, load_model = _training_runtime()
    from mlx.utils import tree_flatten, tree_unflatten
    started = time.monotonic()
    limit = float(TRAIN_CONFIG["max_seconds"])
    elapsed_offset = float(state.get("elapsed_seconds", 0.0)) if state else 0.0
    stop = lambda: elapsed_offset + time.monotonic() - started >= limit
    context = hashlib.sha256(json.dumps({"protocol": frozen, "rows": hashes}, sort_keys=True).encode()).hexdigest()
    model, tokenizer = load_model(start_from_v4=True)
    cache_train = target / "features" / "train"
    cache_validation = target / "features" / "validation"
    optimizer = optim.AdamW(learning_rate=TRAIN_CONFIG["learning_rate"], weight_decay=0)
    optimizer.init(model.trainable_parameters())
    if resume:
        _restore_adapter(model, resume_checkpoint, frozen)
        optimizer.state = tree_unflatten(list(mx.load(str(resume_checkpoint / "optimizer.npz")).items()))
        mx.random.state = [mx.load(str(resume_checkpoint / "rng.npz"))["state"]]
        train_index_hash, validation_index_hash = state["feature_hashes"]["train"], state["feature_hashes"]["validation"]
        state["status"] = "running"
    else:
        train_index_hash = feature_executor._cache_features(model, tokenizer, _ordered(train_rows), cache_train, context, stop)
        validation_index_hash = None if train_index_hash is None else feature_executor._cache_features(model, tokenizer, _ordered(validation_rows), cache_validation, context, stop)
        if train_index_hash is None or validation_index_hash is None:
            result = {"protocol": "som-v1", "domain": domain, "status": "prep_time_limit", "audit": audit, "row_hashes": hashes,
                      "frozen_protocol_sha256": _frozen_hash(frozen), "context_sha256": context, "elapsed_seconds": elapsed_offset + time.monotonic() - started}
            write_json(target / "training_summary.json", result)
            return result
        initial = feature_executor._metrics(model, tokenizer, _ordered(validation_rows), cache_validation)
        state = {"protocol": "som-v1", "domain": domain, "status": "running", "audit": audit, "row_hashes": hashes,
                 "frozen_protocol_sha256": _frozen_hash(frozen), "context_sha256": context,
                 "feature_hashes": {"train": train_index_hash, "validation": validation_index_hash}, "validation": {"0": initial},
                 "history": [], "checkpoints": [], "samples_seen": 0, "updates": 0, "elapsed_seconds": elapsed_offset}

    def checkpoint(label):
        path = target / "checkpoints" / f"{state['samples_seen']:06d}-{label:03d}-{uuid.uuid4().hex[:8]}"
        _save_adapter(model, path, parent=V4_ADAPTER_SHA256, frozen=frozen)
        state["checkpoints"].append(str(path.relative_to(target)))
        mx.eval(optimizer.state)
        mx.savez(str(path / "optimizer.npz"), **dict(tree_flatten(optimizer.state)))
        mx.savez(str(path / "rng.npz"), state=mx.random.state[0])
        write_json(path / "trainer.json", state)
        write_json(target / "latest.json", {"checkpoint": str(path.relative_to(target))})
        feature_executor._check_memory()
        return path

    if not resume:
        checkpoint(0)
    gradient = nn.value_and_grad(model, feature_executor._loss)
    losses = []
    component_history = []
    # Fractions are sample counts, not optimizer-step counts.  With batch one
    # and accumulation eight, splitting the boundary group is required for the
    # exact 300/600/900/1200 frontend checkpoints.
    marks = checkpoint_marks(len(train_rows))
    ordered_train = _ordered(train_rows)
    ordered_validation = _ordered(validation_rows)
    for group in checkpoint_groups(state["samples_seen"], len(ordered_train), TRAIN_CONFIG["gradient_accumulation"], tuple(marks)):
        if stop():
            state["status"] = "interrupted"
            break
        accumulated = None
        for index in group:
            feature = feature_executor._load_feature(cache_train, index)
            count = int(feature["count"].item())
            raw = [model.score_features(feature[f"hidden{i}"], feature["labels"], feature[f"candidate_end{i}"])[0] @ mx.array([1., -1.]) for i in range(count)]
            mx.eval(*raw)
            component_history.append(loss_component_accounting([float(value.item()) for value in raw], int(feature["gold"].item()), bool(feature["missing"].item()), np.asarray(feature["prior"]), float(feature["candidate_positive_weight"].item())))
            value, grads = gradient(model, feature)
            mx.eval(value, grads)
            losses.append(float(value.item()))
            accumulated = grads if accumulated is None else tree_map(lambda a, b: a + b, accumulated, grads)
        optimizer.update(model, tree_map(lambda value: value / len(group), accumulated))
        mx.eval(model.parameters(), optimizer.state)
        feature_executor._check_memory()
        state["samples_seen"] += len(group)
        state["updates"] += 1
        if state["samples_seen"] in marks:
            measurement = feature_executor._metrics(model, tokenizer, ordered_validation, cache_validation)
            state["validation"][str(state["samples_seen"])] = measurement
            # ``specialist_loss`` remains unchanged.  These named totals make
            # its existing components visible for the next objective review.
            state["history"].append({"samples_seen": state["samples_seen"], "loss": float(np.mean(losses)),
                                     "loss_components": {key: float(np.mean([item[key] for item in component_history])) for key in component_history[0]},
                                     "validation": measurement})
            checkpoint(marks[state["samples_seen"]])
    else:
        state["status"] = "completed"
    state["elapsed_seconds"] = elapsed_offset + time.monotonic() - started
    checkpoint(1000 if state["status"] == "completed" else 999)
    selected_seen = min((float(value["loss"]), int(seen)) for seen, value in state["validation"].items())[1]
    selected = next(path for path in state["checkpoints"] if int(path.split("/")[-1].split("-")[0]) == selected_seen)
    write_json(target / "selected.json", {"checkpoint": selected, "selection": "lowest validation loss only; final was not read"})
    write_json(target / "training_summary.json", state)
    return state
