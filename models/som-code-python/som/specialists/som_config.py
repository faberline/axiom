"""Frozen SOM v1 paths and training limits."""
from pathlib import Path

from ..paths import ROOT, read_json, sha256

PROTOCOL = "som-v1"
DOMAINS = ("frontend", "python")
TRAIN_DOMAINS = ("frontend", "python", "mixed")
SPLITS = ("train", "validation", "calibration", "final")
DATA = ROOT / "data" / "som" / "frontend-v1"
MIXED_CONTROL = ROOT / "data" / "som" / "mixed-control"
RUNS = ROOT / "runs" / "som"
SEED = ROOT / "models" / "som-seeds" / "v4"
SOURCE_LOCK = DATA / "sources.lock.json"
MODEL_PATH = ROOT / "models" / "qwen3.5-4b-4bit"
MAX_TOKENS = 1536
REVIEW_ID = "__review__"
REVIEW_TEXT = "No supplied patch is safe enough; request human review."
MODEL_CONFIG = {
    "architecture": "qwen35_independent_candidate_verifier",
    "rank": 8, "scale": 16.0, "seed": 47, "max_tokens": MAX_TOKENS,
    "thinking": False, "review_logit": 0.0,
    "lora_layers": "last full-attention block; Q/V only; detached frozen prefix",
}
TRAIN_CONFIG = {
    "learning_rate": 3e-5, "batch_size": 1, "gradient_accumulation": 8,
    "max_epochs": 1, "max_seconds": 7200, "memory_gib": 24,
    "checkpoint_fractions": (0.0, 0.25, 0.50, 0.75, 1.0),
    "smoke_rows": 32, "smoke_min_accuracy": .90, "smoke_min_loss_reduction": .25,
}
GATES = {
    "accuracy": .80, "present_accuracy": .85, "missing_review_recall": .90,
    "capability_accuracy": .70, "coverage": .25, "accepted": 40,
    "wilson_error_upper": .10, "order_consistency": 1.0,
    "paraphrase_consistency": .95, "reload_max_error": 1e-6,
    "p95_three_seconds": 8., "p95_seven_seconds": 15.,
}


def run_path(domain: str) -> Path:
    if domain not in TRAIN_DOMAINS:
        raise ValueError("SOM domain must be frontend, python, or mixed.")
    return RUNS / domain


def corpus_path(domain: str) -> Path:
    """Return the isolated corpus location for one deployable specialist.

    ``mixed-control`` is an evaluation-only asset. It is deliberately absent
    from this routing table so Python training cannot consume its rows.
    """
    if domain == "frontend":
        return DATA
    if domain == "python":
        return ROOT / "data" / "som" / "python-v1"
    raise ValueError("SOM corpus domain must be frontend or python.")


def corpus_source_lock(domain: str) -> Path:
    return corpus_path(domain) / "sources.lock.json"


def source_lock_sha256() -> str:
    return sha256(SOURCE_LOCK)


def verify_som_seed() -> dict:
    manifest_path = SEED / "manifest.json"
    if not manifest_path.exists():
        raise FileNotFoundError(f"SOM seed manifest is missing: {manifest_path}")
    manifest = read_json(manifest_path)
    adapter = SEED / "adapter.safetensors"
    expected = manifest.get("files", {}).get("adapter.safetensors", {}).get("sha256")
    actual = sha256(adapter)
    if manifest.get("asset") != "som-v4-seed" or actual != expected:
        raise ValueError("SOM seed hash differs from its manifest.")
    return {"seed": str(SEED), "adapter_sha256": actual, "manifest_sha256": sha256(manifest_path)}


# Internal training code uses these names while it is being migrated. They
# point only to the SOM seed and are not a public compatibility surface.
V4_CHECKPOINT = SEED
V4_ADAPTER_SHA256 = read_json(SEED / "manifest.json")["files"]["adapter.safetensors"]["sha256"]
verify_v4_start = verify_som_seed
CONTROL_ROW_HASHES = {
    split: {domain: item["sha256"] for name, item in read_json(MIXED_CONTROL / "manifest.json")["files"].items()
            for domain in ("python", "rust") if name == f"{split}-{domain}.jsonl"}
    for split in SPLITS
}
