import hashlib
import json
import random
import unicodedata
from collections import defaultdict

from .assets import load_raw
from .paths import ROOT, model_path, write_json, sha256
from .schema import encode

QUESTIONS = {
    "banking77": "Which banking support intent best matches this customer message?",
    "emotion": "Which emotion is expressed in this text?",
    "ag_news": "Which news topic best describes this article?",
}
PARAPHRASES = {
    "banking77": "Select the banking issue that the customer wants help with.",
    "emotion": "Select the feeling that the writer expresses.",
    "ag_news": "Select the category that fits the news report.",
}


def text_key(text):
    normalized = " ".join(unicodedata.normalize("NFKC", text).casefold().split())
    return hashlib.sha256(normalized.encode()).hexdigest()


def make_example(row, task, labels, rng):
    gold = int(row["label"])
    size = 4 if task == "ag_news" else rng.randint(2, min(8, len(labels)))
    others = [i for i in range(len(labels)) if i != gold]
    indices = [gold] + rng.sample(others, size - 1)
    rng.shuffle(indices)
    return {
        "id": row["source_id"], "task": task, "text_sha256": text_key(row["text"]),
        "state": row["text"], "question": QUESTIONS[task],
        "candidates": [{"id": str(i), "text": labels[i]} for i in indices],
        "gold_candidate_id": str(gold),
    }


def stratified_order(rows, rng):
    groups = defaultdict(list)
    for row in rows:
        groups[int(row["label"])].append(row)
    for group in groups.values():
        rng.shuffle(group)
    ordered = []
    while groups:
        keys = sorted(groups)
        rng.shuffle(keys)
        for key in keys:
            ordered.append(groups[key].pop())
            if not groups[key]:
                del groups[key]
    return ordered


def prepare():
    from transformers import AutoTokenizer
    tokenizer = AutoTokenizer.from_pretrained(model_path(), local_files_only=True)
    raw, labels = load_raw()
    rng = random.Random(42)
    output = {name: [] for name in ("train", "validation", "calibration", "test", "unseen")}
    used = set()
    stats = {"duplicates_removed": 0, "too_long_removed": 0}
    # Reserve all official test text, not only the selected test subset.
    test_text = {text_key(r["text"]) for (task, split), rows in raw.items() if split == "test" for r in rows}
    validation_text = {text_key(r["text"]) for (task, split), rows in raw.items() if split == "validation" for r in rows}
    plans = [
        ("banking77", "test", "test", 500), ("emotion", "test", "test", 500),
        ("ag_news", "test", "unseen", 400),
        ("emotion", "validation", "validation", 200),
        ("emotion", "validation", "calibration", 200),
        ("banking77", "train", "validation", 200),
        ("banking77", "train", "calibration", 200),
        ("banking77", "train", "train", 1000), ("emotion", "train", "train", 1000),
    ]
    for task, split, destination, count in plans:
        selected = []
        for row in stratified_order(raw[(task, split)], rng):
            key = text_key(row["text"])
            if key in used or (split != "test" and key in test_text) or (split == "train" and key in validation_text):
                stats["duplicates_removed"] += 1
                continue
            example = make_example(row, task, labels[task], rng)
            try:
                encode(example, tokenizer)
            except ValueError:
                stats["too_long_removed"] += 1
                continue
            selected.append(example)
            used.add(key)
            if len(selected) == count:
                break
        if len(selected) != count:
            raise RuntimeError(f"Not enough eligible rows for {task}/{destination}: {len(selected)} < {count}")
        output[destination].extend(selected)
    for split, examples in output.items():
        rng.shuffle(examples)
        path = ROOT / "data" / f"{split}.jsonl"
        path.write_text("".join(json.dumps(e, ensure_ascii=False) + "\n" for e in examples))
    write_json(ROOT / "data" / "manifest.json", {
        "seed": 42, "max_tokens": 512, "counts": {k: len(v) for k, v in output.items()},
        "filter_counts": stats, "sampling": "Round-robin stratified labels; fixed candidate sets, 2–8 candidates.",
        "sha256": {s: sha256(ROOT / "data" / f"{s}.jsonl") for s in output},
        "downloads_sha256": sha256(ROOT / "data" / "downloads.json"),
        "sources_lock_sha256": sha256(ROOT / "sources.lock.json"),
        "unseen_definition": "AG News is excluded from this fine-tuning. Pretraining exposure is unknown.",
    })
    print(json.dumps({k: len(v) for k, v in output.items()}), flush=True)
