"""MLX Q/V LoRA verifier.  It shares the frozen pinned Qwen base."""
import math
from pathlib import Path

import mlx.core as mx
from mlx.utils import tree_flatten
from mlx_lm import load

from ..model import setup_memory
from ..paths import read_json, resolve_checkpoint, sha256, write_json
from ..developer.modern_model import ModernDecision
from .som_config import MODEL_CONFIG, MODEL_PATH, SEED, V4_ADAPTER_SHA256, source_lock_sha256, verify_som_seed
from .input import prompt, stable_scores

def load_model(checkpoint=None, *, start_from_v4=False):
    setup_memory(); mx.random.seed(MODEL_CONFIG["seed"])
    base, tokenizer = load(str(MODEL_PATH), tokenizer_config={"local_files_only": True})
    model = ModernDecision(base)
    # The SOM seed is the fixed parent adapter for the comparison baseline.
    if start_from_v4:
        verify_som_seed()
        restore(model, SEED, allow_seed=True)
    if checkpoint is not None and Path(checkpoint).resolve() != SEED.resolve():
        restore(model, checkpoint)
    mx.eval(model.parameters()); model.eval()
    return model, tokenizer

def save(model, destination, *, parent_adapter_sha256=None):
    destination = Path(destination); destination.mkdir(parents=True, exist_ok=True)
    mx.save_safetensors(str(destination / "adapter.safetensors"), dict(tree_flatten(model.trainable_parameters())))
    write_json(destination / "model_config.json", {**MODEL_CONFIG, "protocol": "som-v1", "source_lock_sha256": source_lock_sha256(),
                                                     "parent_adapter_sha256": parent_adapter_sha256})

def restore(model, destination, *, allow_seed=False):
    destination = resolve_checkpoint(destination); config = read_json(destination / "model_config.json")
    if not allow_seed and config.get("protocol") != "som-v1":
        raise ValueError("Adapter model protocol differs from SOM v1.")
    if not allow_seed and config.get("parent_adapter_sha256") != V4_ADAPTER_SHA256:
        raise ValueError("Adapter parent differs from the SOM seed.")
    weights = mx.load(str(destination / "adapter.safetensors")); expected = dict(tree_flatten(model.trainable_parameters()))
    if weights.keys() != expected.keys() or any(weights[key].shape != expected[key].shape for key in expected):
        raise ValueError("Adapter parameter names or shapes differ.")
    model.load_weights(list(weights.items()), strict=False); mx.eval(model.parameters())

class Scorer:
    def __init__(self, model, tokenizer): self.model, self.tokenizer = model, tokenizer
    def score_candidate(self, request, candidate):
        tokens, labels, candidate_end = prompt(request, candidate, self.tokenizer, include_candidate_boundary=True)
        values = self.model(mx.array([tokens]), labels, candidate_end)[0]; mx.eval(values)
        score = float((values[0] - values[1]).item())
        if not math.isfinite(score): raise RuntimeError("Non-finite candidate score.")
        return score
    def score(self, request):
        canonical = sorted(request.candidates, key=lambda c: (c.text, c.id))
        mapped = {candidate.id: score for candidate, score in zip(canonical, stable_scores(canonical, [self.score_candidate(request, candidate) for candidate in canonical]))}
        return [mapped[candidate.id] for candidate in request.candidates]
