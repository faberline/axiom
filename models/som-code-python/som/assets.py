"""Download immutable public sources. No remote inference or credentials are used."""
import csv
import json
import urllib.request
from pathlib import Path

from .paths import ROOT, model_path, sources, write_json, sha256


def download():
    from huggingface_hub import snapshot_download, hf_hub_download
    lock = sources()
    dest = model_path()
    print("Downloading pinned Qwen3-0.6B weights.", flush=True)
    snapshot_download(lock["model"]["repo"], revision=lock["model"]["revision"],
                      local_dir=dest, token=False,
                      allow_patterns=["*.json", "*.safetensors", "*.txt", "LICENSE", "README.md"])
    raw = ROOT / "data" / "raw"
    raw.mkdir(parents=True, exist_ok=True)
    bank = lock["banking77"]
    for name in ("categories.json", "train.csv", "test.csv"):
        path = raw / ("banking77-" + name)
        url = f"https://raw.githubusercontent.com/{bank['repo']}/{bank['revision']}/banking_data/{name}"
        with urllib.request.urlopen(url, timeout=60) as response:
            contents = response.read()
        temporary = path.with_suffix(path.suffix + ".tmp")
        temporary.write_bytes(contents)
        temporary.replace(path)
    for task, filenames in {
        "emotion": [f"split/{s}-00000-of-00001.parquet" for s in ("train", "validation", "test")],
        "ag_news": ["data/test-00000-of-00001.parquet"],
    }.items():
        for filename in filenames:
            hf_hub_download(lock[task]["repo"], filename, repo_type="dataset",
                            revision=lock[task]["revision"], local_dir=raw / task, token=False)
    files = [p for p in dest.iterdir() if p.is_file()]
    files += [p for p in raw.rglob("*") if p.is_file() and ".cache" not in p.parts]
    write_json(ROOT / "data" / "downloads.json", {
        "sources": lock, "sha256": {str(p.relative_to(ROOT)): sha256(p) for p in files}
    })
    print("Pinned model and source files are ready.", flush=True)


def load_raw():
    import pyarrow.parquet as pq
    raw = ROOT / "data" / "raw"
    labels = json.loads((raw / "banking77-categories.json").read_text())
    result = {}
    for split in ("train", "test"):
        with (raw / f"banking77-{split}.csv").open() as f:
            rows = list(csv.DictReader(f))
        result[("banking77", split)] = [
            {"text": r["text"], "label": labels.index(r["category"]),
             "source_id": f"banking77:{split}:{i}"} for i, r in enumerate(rows)
        ]
    for task, splits in (("emotion", ("train", "validation", "test")), ("ag_news", ("test",))):
        folder = "split" if task == "emotion" else "data"
        for split in splits:
            rows = pq.read_table(raw / task / folder / f"{split}-00000-of-00001.parquet").to_pylist()
            result[(task, split)] = [dict(r, source_id=f"{task}:{split}:{i}") for i, r in enumerate(rows)]
    return result, {
        "banking77": [s.replace("_", " ").replace("?", "") for s in labels],
        "emotion": ["sadness", "joy", "love", "anger", "fear", "surprise"],
        "ag_news": ["world news", "sports", "business", "science and technology"],
    }
