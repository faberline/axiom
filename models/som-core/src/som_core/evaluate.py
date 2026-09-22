"""Fixed test sets, independent temperature fitting, and local baselines."""
import copy
import gc
import importlib.metadata
import json
import platform
import random
import re
import time
from pathlib import Path

import mlx.core as mx
import numpy as np
from mlx_lm import load
from mlx_lm.models.cache import make_prompt_cache

from .data import PARAPHRASES
from .metrics import fit_temperature, grouped_metrics, probabilities, summarize
from .model import load_model, encoded_logits, restore_adapter, setup_memory
from .paths import ROOT, model_path, read_json, read_rows, resolve_checkpoint, sha256, verify_data, write_json
from .schema import DecisionRequest, encode


def score(model, tokenizer, row):
    started = time.perf_counter()
    encoded = encode(row, tokenizer)
    logits = encoded_logits(model, encoded).tolist()
    return {"logits": logits, "seconds": time.perf_counter() - started,
            "input_tokens": len(encoded.tokens)}


def collect(rows, scorer, destination):
    # Caller puts outputs in an immutable weight-hash directory.
    if destination.exists():
        print(json.dumps({"evaluation": destination.stem, "reused_saved_results": True}), flush=True)
        return read_json(destination)
    scorer(rows[0])  # Warm up; exclude model loading and first dispatch from latency.
    records = []
    for i, row in enumerate(rows):
        result = scorer(row)
        if not all(np.isfinite(result["logits"])):
            raise RuntimeError("Non-finite evaluation logits.")
        records.append({"id": row["id"], "task": row["task"],
                        "candidate_ids": [c["id"] for c in row["candidates"]],
                        "gold_index": next(j for j, c in enumerate(row["candidates"])
                                           if c["id"] == row["gold_candidate_id"]), **result})
        if (i + 1) % 200 == 0:
            print(json.dumps({"evaluation": destination.stem, "done": i + 1, "total": len(rows)}), flush=True)
    write_json(destination, records)
    return records


class QwenBaseline:
    """Original Qwen, greedy short letter plus a conditional letter score baseline."""
    def __init__(self, max_tokens=512):
        self.max_tokens = max_tokens
        setup_memory()
        self.model, self.tokenizer = load(str(model_path()), tokenizer_config={"local_files_only": True})
        self.model.model.set_dtype(mx.float16)
        self.model.eval()
        mx.eval(self.model.parameters())
        self.letters = [chr(65 + i) for i in range(8)]
        tokens = [self.tokenizer.encode(s, add_special_tokens=False) for s in self.letters]
        if any(len(t) != 1 for t in tokens):
            raise ValueError("Short-label baseline requires single-token letters.")
        self.letter_tokens = [t[0] for t in tokens]

    def __call__(self, row):
        started = time.perf_counter()
        request = DecisionRequest.from_dict(row)
        options = "\n".join(f"{self.letters[i]}: {json.dumps(c.text)}" for i, c in enumerate(request.candidates))
        message = (f"STATE: {json.dumps(request.state)}\nQUESTION: {json.dumps(request.question)}\n"
                   f"CANDIDATES:\n{options}\nReply with only the letter of the best candidate.")
        tokens = self.tokenizer.apply_chat_template(
            [{"role": "user", "content": message}], tokenize=True,
            add_generation_prompt=True, enable_thinking=False)
        if len(tokens) > self.max_tokens:
            raise ValueError(f"Baseline prompt exceeds {self.max_tokens} tokens; do not truncate the comparison.")
        cache = make_prompt_cache(self.model)
        # The embeddings are tied in this pinned model. Project only the last position.
        hidden = self.model.model(mx.array([tokens]), cache=cache)[:, -1, :]
        logits = self.model.model.embed_tokens.as_linear(hidden).astype(mx.float32)[0]
        allowed = logits[mx.array(self.letter_tokens[:len(request.candidates)])]
        next_token = logits.argmax()
        mx.eval(allowed, next_token)
        conditional_scores = allowed.tolist()
        conditional_seconds = time.perf_counter() - started
        generated = []
        text = ""
        match = None
        for _ in range(8):
            token = int(next_token.item())
            if token in self.tokenizer.eos_token_ids:
                break
            generated.append(token)
            text = self.tokenizer.decode(generated).strip()
            match = re.fullmatch(r"([A-H])[.)]?", text)
            if match:
                break
            hidden = self.model.model(mx.array([[token]]), cache=cache)[:, -1, :]
            next_token = self.model.model.embed_tokens.as_linear(hidden)[0].argmax()
            mx.eval(next_token)
        index = self.letters.index(match.group(1)) if match else -1
        if index >= len(request.candidates):
            index = -1
        return {"logits": conditional_scores, "seconds": conditional_seconds,
                "input_tokens": len(tokens), "greedy_index": index, "greedy_text": text,
                "greedy_seconds": time.perf_counter() - started, "generated_tokens": len(generated)}


def robustness(model, tokenizer, rows, temperature):
    rng = random.Random(4242)
    selected = sum(([r for r in rows if r["task"] == task][:n]
                    for task, n in (("banking77", 32), ("emotion", 32), ("ag_news", 16))), [])
    changed, rewritten, missing = [], [], []
    for row in selected:
        original = score(model, tokenizer, row)["logits"]
        choice = row["candidates"][int(np.argmax(original))]["id"]
        p_by_id = dict(zip([c["id"] for c in row["candidates"]], probabilities(original, temperature)))
        for _ in range(5):
            permutation = copy.deepcopy(row)
            # Require a different order, including two-candidate questions.
            while permutation["candidates"] == row["candidates"]:
                rng.shuffle(permutation["candidates"])
            z = score(model, tokenizer, permutation)["logits"]
            prediction = permutation["candidates"][int(np.argmax(z))]["id"]
            p = probabilities(z, temperature)
            changed.append({"id": row["id"], "same_choice": prediction == choice,
                            "correct": prediction == row["gold_candidate_id"],
                            "max_probability_shift": max(abs(float(p[i]) - p_by_id[c["id"]])
                                                         for i, c in enumerate(permutation["candidates"]))})
        paraphrase = {**row, "question": PARAPHRASES[row["task"]]}
        z = score(model, tokenizer, paraphrase)["logits"]
        prediction = row["candidates"][int(np.argmax(z))]["id"]
        rewritten.append({"id": row["id"], "same_choice": prediction == choice,
                          "correct": prediction == row["gold_candidate_id"]})
        if len(row["candidates"]) >= 3:
            absent = {**row, "candidates": [c for c in row["candidates"] if c["id"] != row["gold_candidate_id"]]}
            p = probabilities(score(model, tokenizer, absent)["logits"], temperature)
            missing.append({"id": row["id"], "confidence": float(p.max())})
    too_long = {**selected[0], "state": "This input is deliberately too long. " * 600}
    rejected = False
    try:
        encode(too_long, tokenizer)
    except ValueError:
        rejected = True
    return {"base_questions": len(selected), "shuffles": {
                "n": len(changed), "choice_consistency": float(np.mean([r["same_choice"] for r in changed])),
                "accuracy": float(np.mean([r["correct"] for r in changed])),
                "mean_max_probability_shift": float(np.mean([r["max_probability_shift"] for r in changed]))},
            "paraphrases": {"n": len(rewritten),
                "choice_consistency": float(np.mean([r["same_choice"] for r in rewritten])),
                "accuracy": float(np.mean([r["correct"] for r in rewritten]))},
            "missing_gold": {"n": len(missing), "forced_answer_rate": 1.0,
                "confidence_at_least_0_8": float(np.mean([r["confidence"] >= .8 for r in missing])),
                "limitation": "No rejection class exists. Every valid request returns one supplied candidate."},
            "overlong_rejected": rejected,
            "details": {"shuffles": changed, "paraphrases": rewritten, "missing_gold": missing}}


def random_metrics(records):
    result = {}
    for task in sorted({r["task"] for r in records}):
        rows = [r for r in records if r["task"] == task]
        ks = np.array([len(r["logits"]) for r in rows])
        result[task] = {"n": len(rows), "accuracy": float(np.mean(1 / ks)),
                        "nll": float(np.mean(np.log(ks))), "brier": float(np.mean(1 - 1 / ks)),
                        "method": "Exact expectation for uniform random selection; no sampled draws."}
    return result


def greedy_metrics(records):
    result = {}
    for task in sorted({r["task"] for r in records}):
        rows = [r for r in records if r["task"] == task]
        result[task] = {"n": len(rows),
                        "accuracy": float(np.mean([r["greedy_index"] == r["gold_index"] for r in rows])),
                        "invalid_rate": float(np.mean([r["greedy_index"] < 0 for r in rows])),
                        "latency_ms": {f"p{q}": float(np.percentile([r["greedy_seconds"] * 1000 for r in rows], q))
                                       for q in (50, 95, 99)}}
    return result


def evaluate(run, fresh=False):
    run = Path(run)
    manifest = verify_data()
    checkpoint = resolve_checkpoint(run)
    adapter_hash = sha256(checkpoint / "adapter.safetensors")
    saved_state = read_json(checkpoint / "trainer.json")
    if saved_state["manifest_sha256"] != sha256(ROOT / "data" / "manifest.json"):
        raise ValueError("Evaluation data differs from the data recorded for training.")
    output = run / "evaluation" / f"{adapter_hash[:16]}-{sha256(ROOT / 'data' / 'manifest.json')[:12]}"
    if fresh:
        output = output.with_name(output.name + f"-fresh-{time.time_ns()}")
    output.mkdir(parents=True, exist_ok=True)
    calibration_rows = read_rows(ROOT / "data" / "calibration.jsonl")
    test_rows = read_rows(ROOT / "data" / "test.jsonl") + read_rows(ROOT / "data" / "unseen.jsonl")
    model, tokenizer = load_model()
    model.eval()
    restore_adapter(model, run / "initial")
    initial = collect(test_rows, lambda row: score(model, tokenizer, row), output / "untrained.json")
    restore_adapter(model, checkpoint)
    cal = collect(calibration_rows, lambda row: score(model, tokenizer, row), output / "calibration.json")
    calibration = {**fit_temperature(cal), "adapter_sha256": adapter_hash,
                   "data_sha256": manifest["sha256"]["calibration"], "method": "One positive scalar temperature; argmax unchanged."}
    write_json(checkpoint / "calibration.json", calibration)
    trained = collect(test_rows, lambda row: score(model, tokenizer, row), output / "trained.json")
    temperature = calibration["temperature"]
    if not all(np.argmax(r["logits"]) == np.argmax(probabilities(r["logits"], temperature)) for r in trained):
        raise RuntimeError("Calibration changed a choice.")
    probes = [score(model, tokenizer, row)["logits"] for row in test_rows[:8]]
    # Load a fresh model, not only an adapter into the existing object.
    del model
    gc.collect()
    mx.clear_cache()
    model, tokenizer = load_model()
    restore_adapter(model, checkpoint)
    model.eval()
    reloaded = [score(model, tokenizer, row)["logits"] for row in test_rows[:8]]
    reload_error = max(float(np.max(np.abs(np.asarray(a) - b))) for a, b in zip(probes, reloaded))
    if reload_error > 1e-5:
        raise RuntimeError(f"Reload changed logits by {reload_error}.")
    robustness_file = output / "robustness.json"
    if not robustness_file.exists():
        write_json(robustness_file, robustness(model, tokenizer, test_rows, temperature))
    robust = read_json(robustness_file)
    del model
    gc.collect()
    mx.clear_cache()
    baseline = QwenBaseline()
    qwen_cal = collect(calibration_rows, baseline, output / "qwen-calibration.json")
    qwen = collect(test_rows, baseline, output / "qwen.json")
    qwen_temperature = fit_temperature(qwen_cal)
    trained_metrics = grouped_metrics(trained, temperature)
    report = {
        "checkpoint": str(checkpoint.resolve()), "adapter_sha256": adapter_hash,
        "data_manifest": manifest, "training": read_json(run / "training_summary.json"),
        "calibration": calibration, "qwen_calibration": qwen_temperature,
        "runtime": {"python": platform.python_version(), "platform": platform.platform(),
                    "packages": {p: importlib.metadata.version(p) for p in ("mlx", "mlx-lm", "numpy", "scipy", "transformers")},
                    "evaluation_peak_mlx_memory_gib": mx.get_peak_memory() / 1024**3},
        "random": random_metrics(trained), "untrained_head": grouped_metrics(initial),
        "trained_raw": grouped_metrics(trained), "trained_calibrated": trained_metrics,
        "qwen_greedy_short_label": greedy_metrics(qwen),
        "qwen_conditional_letters_raw": grouped_metrics(qwen),
        "qwen_conditional_letters_calibrated": grouped_metrics(qwen, qwen_temperature["temperature"]),
        "by_candidate_count": {str(k): summarize([r for r in trained if len(r["logits"]) == k], temperature)
                               for k in sorted({len(r["logits"]) for r in trained})},
        "robustness": {k: v for k, v in robust.items() if k != "details"},
        "verification": {"fresh_reload_max_logit_error": reload_error, "calibration_preserves_all_test_choices": True},
        "learning_status": {task: "above_random_in_this_sample" if value["accuracy"] > value["random_accuracy"] else "not_learned"
                            for task, value in trained_metrics.items()},
        "notes": ["BANKING77 and Emotion scores use sampled candidate subsets, not all-label classification.",
                  "AG News is excluded from this fine-tuning; base-model pretraining exposure is unknown.",
                  "Greedy Qwen uses its official chat template with thinking disabled and up to 8 output tokens; invalid labels count as errors.",
                  "Conditional Qwen probabilities normalize only the candidate-letter logits at the first output position.",
                  "Latency is serial, warm model, batch 1, includes prompt encoding and synchronized GPU output, excludes model load.",
                  "MLX allocator limit is 24 GiB. This is not a hard limit on total macOS process memory.",
                  "Calibration is fitted only on the 400 calibration examples; no test-driven hyperparameter search.",
                  "One seed and small balanced test subsets do not establish production reliability or Jev-level performance."]}
    write_json(output / "report.json", report)
    write_json(run / "evaluation.json", {"report": str((output / "report.json").relative_to(run))})
    write_report(report, run / "REPORT.md")
    print(json.dumps({"report": str(run / "REPORT.md"), "learning_status": report["learning_status"],
                      "accuracy": {k: v["accuracy"] for k, v in trained_metrics.items()}}), flush=True)
    return report


def write_report(report, destination):
    lines = ["# SOM 第一輪結果", "", "此報告由 `som evaluate` 產生。數值來自本機實測。", "",
             "| 任務 | 題數 | 隨機期望 | 未訓練評分器 | SOM | 原始 Qwen 短標籤 |", "|---|---:|---:|---:|---:|---:|"]
    for task, values in report["trained_calibrated"].items():
        scores = [report["random"][task]["accuracy"], report["untrained_head"][task]["accuracy"],
                  values["accuracy"], report["qwen_greedy_short_label"][task]["accuracy"]]
        lines.append(f"| {task} | {values['n']} | " + " | ".join(f"{s:.1%}" for s in scores) + " |")
    lines += ["", "各任務的學習判定：", ""]
    for task, status in report["learning_status"].items():
        lines.append(f"- {task}：" + ("本次樣本高於隨機期望。" if status == "above_random_in_this_sample" else "尚未學會。先檢查資料及模型，不自動加訓。"))
    training = report["training"]
    lines += ["", f"訓練狀態：`{training['status']}`。處理 {training['samples_seen']} 筆，共 {training['updates']} 次參數更新。",
              f"訓練計時 {training['elapsed_seconds']:.1f} 秒。MLX 記憶體峰值 {training['peak_memory_gib']:.2f} GiB。",
              "", f"機率校準溫度：{report['calibration']['temperature']:.4f}。所有測試題的選擇均保持相同。", "",
              "## 機率與延遲", "", "NLL 是正確答案的負對數機率。Brier 是機率向量的平方誤差。ECE 是信心與正確率的分箱差距。數值越低越好。", "",
              "前 → 後表示校準前後。校準只最佳化獨立校準集的整體 NLL，不保證每個任務或每個指標都改善。", "",
              "| 任務 | NLL 前 → 後 | Brier 前 → 後 | ECE 前 → 後 | 回應中位數 | 回應 P95 |", "|---|---:|---:|---:|---:|---:|"]
    for task, v in report["trained_calibrated"].items():
        before = report["trained_raw"][task]
        lines.append(f"| {task} | {before['nll']:.3f} → {v['nll']:.3f} | {before['brier']:.3f} → {v['brier']:.3f} | {before['ece_10_bins']:.3f} → {v['ece_10_bins']:.3f} | {v['latency_ms']['p50']:.1f} ms | {v['latency_ms']['p95']:.1f} ms |")
    lines += ["", "原始 Qwen 短標籤的延遲與輸出檢查：", "",
              "| 任務 | 無效標籤比例 | 回應中位數 | 回應 P95 |", "|---|---:|---:|---:|"]
    for task, v in report["qwen_greedy_short_label"].items():
        lines.append(f"| {task} | {v['invalid_rate']:.1%} | {v['latency_ms']['p50']:.1f} ms | {v['latency_ms']['p95']:.1f} ms |")
    lines += ["", "## 信心門檻", "", "覆蓋率是達到門檻的題目比例。誤判率只計算這些題目。這不是自動拒答功能。", "",
              "| 任務 | 門檻 | 達標題數 | 覆蓋率 | 誤判率 |", "|---|---:|---:|---:|---:|"]
    for task, values in report["trained_calibrated"].items():
        for r in values["risk_coverage"]:
            error = "無樣本" if r["error"] is None else f"{r['error']:.1%}"
            lines.append(f"| {task} | {r['threshold']:.2f} | {r['accepted']} | {r['coverage']:.1%} | {error} |")
    robust = report["robustness"]
    lines += ["", "## 邊界檢查", "",
              f"交換候選順序：{robust['shuffles']['n']} 次，選擇保持相同的比例為 {robust['shuffles']['choice_consistency']:.1%}。",
              f"改寫問題：{robust['paraphrases']['n']} 題，選擇保持相同的比例為 {robust['paraphrases']['choice_consistency']:.1%}。",
              f"移除正確答案：{robust['missing_gold']['n']} 題，模型仍會強制選一個答案。信心至少 0.8 的比例為 {robust['missing_gold']['confidence_at_least_0_8']:.1%}。",
              f"超長輸入拒絕：{robust['overlong_rejected']}。重新載入後最大分數差：{report['verification']['fresh_reload_max_logit_error']:.8f}。", "",
              "## 限制", "", "每個任務只跑一個種子。Banking77 的每題最多 8 個候選，不是完整 77 類分類。Emotion 的每題有 2–6 個候選。AG News 固定 4 個候選。", "",
              "AG News 沒有參與本次微調。無法確認基礎模型是否曾見過公開測試資料。", "",
              "保留基礎權重不代表所有原有能力均保持。LoRA 也會改變模型行為。", "",
              "正確率未高於隨機期望的任務會標記為尚未學會。不會自動增加訓練。", "",
              "完整數值、每題分數、原始 Qwen 的條件機率比較及執行版本，見 `evaluation.json` 指向的報告目錄。", ""]
    Path(destination).write_text("\n".join(lines))
