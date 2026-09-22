"""One backbone pass, with a shared scorer for variable candidate sets."""
import math
from pathlib import Path

import mlx.core as mx
import mlx.nn as nn
from mlx.utils import tree_flatten
from mlx_lm import load
from mlx_lm.tuner.lora import LoRALinear

from .paths import ROOT, model_path, read_json, sources, write_json, sha256, resolve_checkpoint
from .schema import DecisionRequest, encode

MODEL_CONFIG = {"rank": 8, "scale": 16.0, "projection_dim": 256, "max_tokens": 512, "seed": 42}


def setup_memory():
    mx.set_memory_limit(24 * 1024**3)
    mx.set_cache_limit(512 * 1024**2)


class DecisionModel(nn.Module):
    def __init__(self, backbone, config=MODEL_CONFIG):
        super().__init__()
        self.model_config = dict(config)
        self.backbone = backbone
        self.backbone.freeze()
        for layer in self.backbone.layers:
            attention = layer.self_attn
            for name in ("q_proj", "v_proj"):
                base = getattr(attention, name)
                lora = LoRALinear.from_base(base, r=config["rank"], scale=config["scale"], dropout=0.0)
                lora.linear.freeze()
                setattr(attention, name, lora)
        dim = backbone.args.hidden_size
        width = config["projection_dim"]
        self.query = nn.Linear(dim, width, bias=False)
        self.key = nn.Linear(dim, width, bias=False)
        self.local = nn.Linear(dim, 1, bias=False)
        self.width = width

    def __call__(self, tokens, positions, decision_position):
        hidden = self.backbone(tokens)
        candidates = hidden[:, positions, :].astype(mx.float32)
        final = hidden[:, decision_position, :].astype(mx.float32)
        q = self.query(final)
        keys = self.key(candidates)
        return (keys * q[:, None, :]).sum(-1) / math.sqrt(self.width) + self.local(candidates).squeeze(-1)


def load_model(config=None):
    config = MODEL_CONFIG if config is None else config
    setup_memory()
    mx.random.seed(config["seed"])
    language_model, tokenizer = load(str(model_path()), tokenizer_config={"local_files_only": True})
    # Run the backbone only. The vocabulary output layer is not used for decisions.
    language_model.model.set_dtype(mx.float16)
    model = DecisionModel(language_model.model, config)
    mx.eval(model.parameters())
    return model, tokenizer


def encoded_logits(model, encoded):
    logits = model(mx.array([encoded.tokens]), encoded.candidate_positions, encoded.decision_position)[0]
    mx.eval(logits)
    return logits


def save_adapter(model, directory):
    directory = Path(directory)
    directory.mkdir(parents=True, exist_ok=True)
    mx.save_safetensors(str(directory / "adapter.safetensors"), dict(tree_flatten(model.trainable_parameters())))
    write_json(directory / "model_config.json", {
        **model.model_config, "base_model": sources()["model"],
        "sources_lock_sha256": sha256(ROOT / "sources.lock.json"),
    })


def restore_adapter(model, directory):
    directory = Path(directory)
    config = read_json(directory / "model_config.json")
    if config["base_model"] != sources()["model"] or any(config[k] != v for k, v in model.model_config.items()):
        raise ValueError("Checkpoint model or architecture does not match this project.")
    weights = mx.load(str(directory / "adapter.safetensors"))
    expected = dict(tree_flatten(model.trainable_parameters()))
    if weights.keys() != expected.keys() or any(weights[k].shape != expected[k].shape for k in expected):
        raise ValueError("Checkpoint adapter names or shapes do not match.")
    model.load_weights(list(weights.items()), strict=False)
    mx.eval(model.parameters())


class Predictor:
    def __init__(self, checkpoint):
        checkpoint = resolve_checkpoint(checkpoint)
        saved_config = read_json(checkpoint / "model_config.json")
        config = {k: saved_config[k] for k in MODEL_CONFIG}
        self.model, self.tokenizer = load_model(config)
        self.max_tokens = config["max_tokens"]
        restore_adapter(self.model, checkpoint)
        self.model.eval()
        calibration = checkpoint / "calibration.json"
        self.temperature = 1.0
        self.calibrated = False
        if calibration.exists():
            value = read_json(calibration)
            if value["adapter_sha256"] != sha256(checkpoint / "adapter.safetensors"):
                raise ValueError("Calibration belongs to different model weights.")
            self.temperature = float(value["temperature"])
            if not math.isfinite(self.temperature) or self.temperature <= 0:
                raise ValueError("Calibration temperature must be finite and positive.")
            self.calibrated = True

    def token_count(self,value):
        return len(encode(value,self.tokenizer,self.max_tokens).tokens)

    def predict(self, value):
        request = DecisionRequest.from_dict(value)
        encoded = encode(request, self.tokenizer, self.max_tokens)
        logits = encoded_logits(self.model, encoded)
        probabilities = mx.softmax(logits / self.temperature)
        mx.eval(probabilities)
        scores, probs = logits.tolist(), probabilities.tolist()
        if not all(math.isfinite(x) for x in scores + probs):
            raise RuntimeError("The model returned non-finite scores.")
        choice = max(range(len(scores)), key=scores.__getitem__)
        return {
            "choice_id": request.candidates[choice].id,
            "candidates": [{"id": c.id, "score": scores[i], "probability": probs[i]} for i, c in enumerate(request.candidates)],
            "calibrated": self.calibrated,
        }
