import hashlib
import json
import random
import re
import subprocess
import sys
import warnings
from collections import Counter

from ..paths import ROOT, model_path, read_json, read_rows, sha256, write_json
from ..schema import encode
from .fixtures import fixtures

DATA = ROOT / "data" / "developer-v1"
MAX_TOKENS = 1024
REVIEW_ID = "__review__"
REVIEW_TEXT = "None of these replacements meets the requirement. Request human review."
QUESTION = "Which replacement meets every stated requirement? Choose human review if none does."
PROTOCOL = {
    "version": 1, "seed": 43, "max_tokens": MAX_TOKENS, "max_epochs": 4,
    "max_training_seconds": 7200, "synthetic_only": True,
    "train_variants_per_family": 48, "validation_variants_per_family": 6,
    "calibration_variants_per_family": 10, "test_variants_per_family": 10,
    "held_out_variants_per_family": 20,
    "gates": {"seen_accuracy_each_domain": .85, "held_out_accuracy_each_domain": .80,
              "accepted_error_wilson_upper": .10, "accepted_min_n_each_domain": 50,
              "accepted_coverage_each_domain": .25, "order_consistency": .95,
              "missing_fix_review_recall": .85},
    "scope": "Rank supplied short Python or JavaScript/DOM/React-style state replacements; human reviews changes.",
    "exclusions": "Not a code generator. No evidence for full repositories, CSS layout, TypeScript type checking, or general developer reliability.",
}


def instantiate(fixture, index, rng):
    number = rng.randint(2, 9)
    key = rng.choice(["value", "amount", "score", "count", "level", "rank", "size", "total"])
    function = rng.choice(["handle", "convert", "update", "prepare", "process", "apply"]) + f"_{rng.randrange(10**9):09d}"
    def render(text):
        text = text.replace("@N@", str(number)).replace("@K@", json.dumps(key)).replace("@KEYTEXT@", key)
        return re.sub(r"\bf\b", function, text)
    return {"id": f"{fixture.domain}:{fixture.name}:{index}", "task": fixture.domain,
            "family": fixture.name, "held_out_family": fixture.held_out,
            "requirement": render(fixture.requirement), "sources": [render(s) for s in fixture.sources],
            "checks": render(fixture.checks), "dom": fixture.dom}


def check_cases(cases):
    outcomes = {}
    with warnings.catch_warnings():
        warnings.simplefilter("ignore", RuntimeWarning)
        for case in cases:
            if case["task"] != "python":
                continue
            passed = []
            for source in case["sources"]:
                namespace = {}
                try:
                    # Only authored fixtures enter this path, never CLI input or fetched code.
                    exec(compile(source + "\n" + case["checks"], case["id"], "exec"), namespace)
                    passed.append(True)
                except Exception:
                    passed.append(False)
            outcomes[case["id"]] = passed
    frontend = [c for c in cases if c["task"] == "frontend"]
    if frontend:
        for start in range(0, len(frontend), 100):
            process = subprocess.run(["node", str(ROOT / "developer-runtime" / "oracle.mjs")],
                input=json.dumps(frontend[start:start + 100]), text=True, capture_output=True,
                timeout=90, cwd=ROOT / "developer-runtime")
            if process.returncode:
                raise RuntimeError(f"Frontend oracle failed: {process.stderr[-1500:]}")
            outcomes.update({r["id"]: r["passed"] for r in json.loads(process.stdout)})
    for case in cases:
        if outcomes.get(case["id"]) != [True, False, False, False]:
            raise ValueError(f"Fixture has ambiguous or incorrect labels: {case['id']} {outcomes.get(case['id'])}")
    return outcomes


def make_row(case, rng, missing):
    # Candidates do not contain a marker revealing the correct implementation.
    wrong = rng.sample(case["sources"][1:], rng.randint(2, 3))
    codes = wrong if missing else [case["sources"][0], *wrong]
    rng.shuffle(codes)
    candidates = [{"id": f"patch-{rng.randrange(10**12):012d}", "text": s} for s in codes]
    gold = REVIEW_ID if missing else next(c["id"] for c in candidates if c["text"] == case["sources"][0])
    candidates.append({"id": REVIEW_ID, "text": REVIEW_TEXT})
    rng.shuffle(candidates)
    state = (f"Language: {'Python 3.12' if case['task'] == 'python' else 'JavaScript frontend'}.\n"
             f"Requirement: {case['requirement']}\nCurrent implementation:\n{rng.choice(case['sources'][1:])}")
    return {"id": case["id"], "task": case["task"], "family": case["family"],
            "held_out_family": case["held_out_family"], "missing_correct_patch": missing,
            "state": state, "question": QUESTION, "candidates": candidates,
            "gold_candidate_id": gold, "text_sha256": hashlib.sha256(state.encode()).hexdigest()}


def prepare():
    from transformers import AutoTokenizer
    tokenizer = AutoTokenizer.from_pretrained(model_path(), local_files_only=True)
    rng = random.Random(PROTOCOL["seed"])
    output = {s: [] for s in ("train", "validation", "calibration", "test", "challenge")}
    cases, row_plan = [], []
    for fixture in fixtures():
        plans = [("challenge", PROTOCOL["held_out_variants_per_family"])] if fixture.held_out else [
            (split, PROTOCOL[f"{split}_variants_per_family"]) for split in ("train", "validation", "calibration", "test")]
        index = 0
        for split, count in plans:
            for i in range(count):
                case = instantiate(fixture, index, rng)
                cases.append(case)
                # Freeze the missing-patch count separately in each split.
                row = make_row(case, rng, missing=i < round(count * .3))
                encode(row, tokenizer, MAX_TOKENS)
                row_plan.append((split, row))
                index += 1
    print(json.dumps({"checking_authored_cases": len(cases), "candidate_programs": len(cases) * 4}), flush=True)
    outcomes = check_cases(cases)
    for split, row in row_plan:
        output[split].append(row)
    DATA.mkdir(parents=True, exist_ok=True)
    for split, rows in output.items():
        rng.shuffle(rows)
        (DATA / f"{split}.jsonl").write_text("".join(json.dumps(row) + "\n" for row in rows))
    write_json(DATA / "oracles.json", cases)
    write_json(DATA / "oracle-results.json", outcomes)
    write_json(DATA / "protocol.json", PROTOCOL)
    write_json(DATA / "manifest.json", {
        "protocol": PROTOCOL, "counts": {s: dict(Counter(r["task"] for r in rows)) for s, rows in output.items()},
        "families": {s: sorted({r["family"] for r in rows}) for s, rows in output.items()},
        "sha256": {s: sha256(DATA / f"{s}.jsonl") for s in output},
        "oracle_sha256": sha256(DATA / "oracles.json"), "oracle_results_sha256": sha256(DATA / "oracle-results.json"),
        "fixtures_sha256": sha256(ROOT / "som/developer/fixtures.py"),
        "generator_sha256": sha256(ROOT / "som/developer/data.py"),
        "node_version": subprocess.check_output(["node", "--version"], text=True).strip(),
        "node_lock_sha256": sha256(ROOT / "developer-runtime/package-lock.json"),
        "python": sys.version, "base_sources_lock_sha256": sha256(ROOT / "som-research-developer.sources.lock.json"),
        "maximum_tokens": max(len(encode(r, tokenizer, MAX_TOKENS).tokens) for rows in output.values() for r in rows),
        "provenance": "Original exercises authored in this project; checked against Python/JavaScript execution and jsdom. No downloaded user code is executed.",
        "references": ["https://docs.python.org/3.12/library/", "https://developer.mozilla.org/en-US/docs/Web/JavaScript",
                       "https://developer.mozilla.org/en-US/docs/Web/API/Node/textContent", "https://react.dev/learn/updating-arrays-in-state"],
    })
    print(json.dumps(verify()), flush=True)


def verify():
    manifest = read_json(DATA / "manifest.json")
    if manifest["protocol"] != PROTOCOL:
        raise ValueError("Developer acceptance protocol changed. Prepare a new experiment.")
    for key, path in (("fixtures_sha256", ROOT / "som/developer/fixtures.py"),
                      ("generator_sha256", ROOT / "som/developer/data.py"),
                      ("oracle_sha256", DATA / "oracles.json"),
                      ("oracle_results_sha256", DATA / "oracle-results.json"),
                      ("node_lock_sha256", ROOT / "developer-runtime/package-lock.json"),
                      ("base_sources_lock_sha256", ROOT / "som-research-developer.sources.lock.json")):
        if sha256(path) != manifest[key]:
            raise ValueError(f"Developer source changed: {key}")
    states, ids, family_sets = set(), set(), {}
    for split, digest in manifest["sha256"].items():
        if sha256(DATA / f"{split}.jsonl") != digest:
            raise ValueError(f"Developer data changed: {split}")
        rows = read_rows(DATA / f"{split}.jsonl")
        family_sets[split] = {r["family"] for r in rows}
        for row in rows:
            if row["id"] in ids or row["text_sha256"] in states:
                raise ValueError("Duplicate developer case across splits.")
            ids.add(row["id"])
            states.add(row["text_sha256"])
            if sum(c["id"] == row["gold_candidate_id"] for c in row["candidates"]) != 1:
                raise ValueError("Missing or duplicate gold candidate.")
    if any(family_sets["challenge"] & family_sets[s] for s in ("train", "validation", "calibration", "test")):
        raise ValueError("Held-out family leaked into development data.")
    return {"status": "passed", "counts": manifest["counts"], "unique_instances": len(ids),
            "training_families": len(family_sets["train"]), "held_out_families": len(family_sets["challenge"]),
            "maximum_tokens": manifest["maximum_tokens"]}


if __name__ == "__main__":
    prepare()
