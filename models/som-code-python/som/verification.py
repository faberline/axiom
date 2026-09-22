"""Verify source hashes, official split boundaries, labels, and API inputs."""
from collections import Counter

from .assets import load_raw
from .data import text_key
from .paths import ROOT, model_path, read_json, read_rows, sha256, sources, verify_data
from .schema import encode


def verify():
    from transformers import AutoTokenizer
    manifest = verify_data()
    downloaded = read_json(ROOT / "data" / "downloads.json")
    if downloaded["sources"] != sources() or sha256(ROOT / "data" / "downloads.json") != manifest["downloads_sha256"]:
        raise ValueError("Download manifest changed.")
    for relative, digest in downloaded["sha256"].items():
        if sha256(ROOT / relative) != digest:
            raise ValueError(f"Source file changed: {relative}")
    raw, labels = load_raw()
    originals = {r["source_id"]: r for rows in raw.values() for r in rows}
    official_test = {text_key(r["text"]) for (_, split), rows in raw.items() if split == "test" for r in rows}
    official_validation = {text_key(r["text"]) for (_, split), rows in raw.items() if split == "validation" for r in rows}
    tokenizer = AutoTokenizer.from_pretrained(model_path(), local_files_only=True)
    seen = set()
    expected = {"train": {"banking77": 1000, "emotion": 1000},
                "validation": {"banking77": 200, "emotion": 200},
                "calibration": {"banking77": 200, "emotion": 200},
                "test": {"banking77": 500, "emotion": 500}, "unseen": {"ag_news": 400}}
    sizes = Counter()
    max_tokens = 0
    for split, counts in expected.items():
        rows = read_rows(ROOT / "data" / f"{split}.jsonl")
        if Counter(r["task"] for r in rows) != counts:
            raise ValueError(f"Wrong sample counts in {split}.")
        for row in rows:
            source = originals[row["id"]]
            key = text_key(row["state"])
            if row["state"] != source["text"] or row["text_sha256"] != key or key in seen:
                raise ValueError(f"Changed or duplicate text: {row['id']}")
            seen.add(key)
            if split in ("test", "unseen"):
                if row["id"].split(":")[1] != "test":
                    raise ValueError("Test row is not from official test split.")
            elif key in official_test or (split == "train" and key in official_validation):
                raise ValueError("Official split boundary crossed.")
            if row["gold_candidate_id"] != str(source["label"]):
                raise ValueError("Gold label does not match original data.")
            if sum(c["id"] == row["gold_candidate_id"] for c in row["candidates"]) != 1:
                raise ValueError("Correct answer must appear exactly once.")
            if any(c["text"] != labels[row["task"]][int(c["id"])] for c in row["candidates"]):
                raise ValueError("Candidate label text does not match the source.")
            sizes[len(row["candidates"])] += 1
            max_tokens = max(max_tokens, len(encode(row, tokenizer).tokens))
    return {"status": "passed", "unique_examples": len(seen), "counts": expected,
            "candidate_counts": dict(sorted(sizes.items())), "maximum_input_tokens": max_tokens,
            "source_files_verified": len(downloaded["sha256"])}
