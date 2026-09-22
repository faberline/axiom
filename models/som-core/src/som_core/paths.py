import hashlib
import json
import os
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
os.environ.setdefault("HF_HOME", str(ROOT / ".cache" / "huggingface"))
os.environ.setdefault("HF_HUB_DISABLE_TELEMETRY", "1")
os.environ.setdefault("TOKENIZERS_PARALLELISM", "false")


def read_json(path):
    p = Path(path)
    if not p.exists():
        return {}
    return json.loads(p.read_text())


def write_json(path, value):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(json.dumps(value, indent=2, ensure_ascii=False, allow_nan=False) + "\n")
    tmp.replace(path)


def sha256(path):
    p = Path(path)
    if not p.exists():
        return ""
    with p.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def read_rows(path):
    p = Path(path)
    if not p.exists():
        return []
    return [json.loads(line) for line in p.read_text().splitlines() if line.strip()]


def model_path():
    return ROOT / "models" / "qwen3-0.6b"


def sources():
    path = ROOT / "sources.lock.json"
    if path.exists():
        return read_json(path)
    return {"model": "mlx-community/Qwen2.5-Coder-0.5B-Instruct-4bit"}


def resolve_checkpoint(path):
    path = Path(path)
    if (path / "selected.json").exists():
        return path / read_json(path / "selected.json")["checkpoint"]
    if (path / "latest.json").exists():
        return path / read_json(path / "latest.json")["checkpoint"]
    return path


def verify_data():
    manifest_path = ROOT / "data" / "manifest.json"
    if not manifest_path.exists():
        return {}
    manifest = read_json(manifest_path)
    for split, digest in manifest.get("sha256", {}).items():
        split_file = ROOT / "data" / f"{split}.jsonl"
        if split_file.exists() and sha256(split_file) != digest:
            raise ValueError(f"Prepared data changed: {split}. Run prepare again.")
    return manifest
