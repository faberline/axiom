import hashlib
import json
import os
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
os.environ.setdefault("HF_HOME", str(ROOT / ".cache" / "huggingface"))
os.environ.setdefault("HF_HUB_DISABLE_TELEMETRY", "1")
os.environ.setdefault("TOKENIZERS_PARALLELISM", "false")


def read_json(path):
    return json.loads(Path(path).read_text())


def write_json(path, value):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(json.dumps(value, indent=2, ensure_ascii=False, allow_nan=False) + "\n")
    tmp.replace(path)


def sha256(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def read_rows(path):
    return [json.loads(line) for line in Path(path).read_text().splitlines() if line.strip()]


def model_path():
    return ROOT / "models" / "qwen3-0.6b"


def sources():
    return read_json(ROOT / "sources.lock.json")


def resolve_checkpoint(path):
    path = Path(path)
    if (path / "selected.json").exists():
        return path / read_json(path / "selected.json")["checkpoint"]
    if (path / "latest.json").exists():
        return path / read_json(path / "latest.json")["checkpoint"]
    return path


def verify_data():
    manifest = read_json(ROOT / "data" / "manifest.json")
    for split, digest in manifest["sha256"].items():
        if sha256(ROOT / "data" / f"{split}.jsonl") != digest:
            raise ValueError(f"Prepared data changed: {split}. Run prepare again.")
    if manifest["sources_lock_sha256"] != sha256(ROOT / "sources.lock.json"):
        raise ValueError("Source lock changed. Download and prepare again.")
    return manifest
