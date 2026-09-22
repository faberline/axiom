import copy
import gc
import json
import time
from pathlib import Path

import mlx.core as mx
import numpy as np

from ..evaluate import QwenBaseline, collect, greedy_metrics, random_metrics
from ..metrics import fit_temperature, summarize
from ..model import load_model, encoded_logits, restore_adapter
from ..paths import ROOT, read_json, read_rows, resolve_checkpoint, sha256, write_json
from ..schema import encode
from .data import DATA, MAX_TOKENS, PROTOCOL, REVIEW_ID, verify
from .policy import choose_threshold, selective_metrics
from .train import RUN, CONFIG
from .external import DIRECTORY as TRANSFER_DIRECTORY, rows as transfer_rows


def paired_score(model, tokenizer, row):
    started = time.perf_counter()
    item = encode(row, tokenizer, MAX_TOKENS)
    logits = encoded_logits(model, item).tolist()
    first_seconds = time.perf_counter() - started
    reverse = {**row, "candidates": list(reversed(row["candidates"]))}
    reversed_logits = encoded_logits(model, encode(reverse, tokenizer, MAX_TOKENS)).tolist()
    return {"logits": logits, "seconds": first_seconds, "input_tokens": len(item.tokens),
            "reverse_logits": reversed_logits, "reverse_candidate_ids": [c["id"] for c in reverse["candidates"]],
            "paired_seconds": time.perf_counter() - started}


def enrich(records, rows):
    indexed = {r["id"]: r for r in rows}
    return [{**r, "family": indexed[r["id"]]["family"],
            "missing_correct_patch": indexed[r["id"]]["missing_correct_patch"]} for r in records]


def runtime_order(rows):
    # Match DeveloperPredictor: supplied patches first, appended review option last.
    return [{**r, "candidates": [c for c in r["candidates"] if c["id"] != REVIEW_ID]
             + [c for c in r["candidates"] if c["id"] == REVIEW_ID]} for r in rows]


def evaluate(run=RUN):
    verify()
    run = Path(run)
    training = read_json(run / "training_summary.json")
    if training["status"] != "completed":
        raise ValueError("Complete the bounded developer run before final evaluation.")
    if training["manifest_sha256"] != sha256(DATA / "manifest.json"):
        raise ValueError("Developer training and evaluation data do not match.")
    checkpoint = resolve_checkpoint(run)
    digest = sha256(checkpoint / "adapter.safetensors")
    output = run / "evaluation" / digest[:16]
    output.mkdir(parents=True, exist_ok=True)
    cal_rows = runtime_order(read_rows(DATA / "calibration.jsonl"))
    test_rows = runtime_order(read_rows(DATA / "test.jsonl"))
    challenge_rows = runtime_order(read_rows(DATA / "challenge.jsonl"))
    all_rows = test_rows + challenge_rows
    model, tokenizer = load_model(CONFIG)
    restore_adapter(model, checkpoint)
    model.eval()
    cal = collect(cal_rows, lambda r: paired_score(model,tokenizer,r), output / "calibration.json")
    global_temperature = fit_temperature(cal)
    policy = {"adapter_sha256": digest, "calibration_sha256": sha256(DATA / "calibration.jsonl"),
              "acceptance_passed": False, "validated_max_tokens": read_json(DATA / "manifest.json")["maximum_tokens"], "domains": {}}
    calibration_details = {}
    for domain in ("python", "frontend"):
        records = [r for r in cal if r["task"] == domain]
        fitted = fit_temperature(records)
        threshold = choose_threshold(records, fitted["temperature"])
        policy["domains"][domain] = {"temperature": fitted["temperature"], "threshold": threshold["selected"]["threshold"]}
        calibration_details[domain] = {**fitted, **threshold}
    # Freeze all probability and threshold choices before reading test predictions.
    write_json(output / "calibration-policy-before-test.json", policy)
    write_json(checkpoint / "calibration.json", {**global_temperature, "adapter_sha256": digest,
               "by_domain": policy["domains"], "data_sha256": sha256(DATA / "calibration.jsonl")})
    test = enrich(collect(test_rows, lambda r: paired_score(model,tokenizer,r), output / "test.json"), test_rows)
    challenge = enrich(collect(challenge_rows, lambda r: paired_score(model,tokenizer,r), output / "challenge.json"), challenge_rows)
    external_rows = runtime_order(transfer_rows())
    external = collect(external_rows, lambda r: paired_score(model,tokenizer,r), output / "public-transfer.json")
    combined = test + challenge
    # Save/reload acceptance uses a new backbone object.
    probes = [paired_score(model,tokenizer,r)["logits"] for r in test_rows[:4]]
    del model
    gc.collect()
    mx.clear_cache()
    model, tokenizer = load_model(CONFIG)
    restore_adapter(model, checkpoint)
    model.eval()
    reloaded = [paired_score(model,tokenizer,r)["logits"] for r in test_rows[:4]]
    reload_error = max(float(np.max(np.abs(np.array(a)-b))) for a,b in zip(probes,reloaded))
    if reload_error > 1e-5:
        raise RuntimeError("Fresh developer model reload changed outputs.")
    paraphrase_rows = [{**r, "question": "Select the valid code change. If all supplied changes fail the requirement, request review."}
                       for r in all_rows[:40] + challenge_rows[:40]]
    paraphrases = collect(paraphrase_rows, lambda r: paired_score(model,tokenizer,r), output / "paraphrases.json")
    initial_path = output / "untrained.json"
    restore_adapter(model, run / "initial")
    initial = collect(all_rows, lambda r: paired_score(model,tokenizer,r), initial_path)
    del model
    gc.collect()
    mx.clear_cache()
    baseline = QwenBaseline(max_tokens=MAX_TOKENS)
    qwen = collect(all_rows, baseline, output / "qwen.json")
    qwen_external = collect(external_rows, baseline, output / "qwen-public-transfer.json")
    metrics, gates = {}, []
    required = PROTOCOL["gates"]
    for domain in ("python", "frontend"):
        t = policy["domains"][domain]["temperature"]
        threshold = policy["domains"][domain]["threshold"]
        seen = [r for r in test if r["task"] == domain]
        held = [r for r in challenge if r["task"] == domain]
        seen_score, held_score = summarize(seen,t), summarize(held,t)
        selective = selective_metrics(seen, t, threshold)
        combined_domain = [r for r in combined if r["task"] == domain]
        robustness = selective_metrics(combined_domain,t,threshold)
        qwen_domain = [r for r in qwen if r["task"] == domain]
        qwen_seen = [r for r in qwen_domain if r["id"] in {s["id"] for s in seen}]
        qwen_held = [r for r in qwen_domain if r["id"] in {s["id"] for s in held}]
        metrics[domain] = {"seen_raw": summarize(seen), "seen_calibrated": seen_score,
            "held_out_raw": summarize(held), "held_out_calibrated": held_score,
            "selective_seen": selective, "selective_held_out": selective_metrics(held,t,threshold),
            "combined_robustness": robustness,
            "untrained": summarize([r for r in initial if r["task"] == domain]),
            "qwen_seen": greedy_metrics(qwen_seen)[domain], "qwen_held_out": greedy_metrics(qwen_held)[domain],
            "public_transfer": summarize([r for r in external if r["task"]==domain],t),
            "public_transfer_selective": selective_metrics([r for r in external if r["task"]==domain],t,threshold),
            "qwen_public_transfer": greedy_metrics([r for r in qwen_external if r["task"]==domain])[domain],
            "always_review_accuracy": float(np.mean([r["candidate_ids"][r["gold_index"]] == REVIEW_ID for r in combined_domain])),
            "paired_latency_ms": {f"p{q}":float(np.percentile([r["paired_seconds"]*1000 for r in combined_domain],q)) for q in (50,95)},
            "by_family": {family:summarize([r for r in combined_domain if r["family"]==family],t)
                          for family in sorted({r["family"] for r in combined_domain})}}
        values = [
            ("seen_accuracy", seen_score["accuracy"], ">=", required["seen_accuracy_each_domain"]),
            ("held_out_accuracy", held_score["accuracy"], ">=", required["held_out_accuracy_each_domain"]),
            ("accepted_error_upper", selective["error_wilson_upper_95"], "<=", required["accepted_error_wilson_upper"]),
            ("accepted_count", selective["accepted"], ">=", required["accepted_min_n_each_domain"]),
            ("accepted_coverage", selective["coverage"], ">=", required["accepted_coverage_each_domain"]),
            ("order_consistency", robustness["order_consistency"], ">=", required["order_consistency"]),
            ("missing_fix_review_recall", robustness["missing_fix_review_recall"], ">=", required["missing_fix_review_recall"]),
        ]
        for name,value,operator,target in values:
            gates.append({"domain":domain,"gate":name,"actual":value,"operator":operator,"target":target,
                          "passed": value >= target if operator == ">=" else value <= target})
    policy["acceptance_passed"] = all(g["passed"] for g in gates)
    write_json(checkpoint / "developer_policy.json", policy)
    originals = {r["id"]: r for r in combined}
    same = sum(r["candidate_ids"][int(np.argmax(r["logits"]))] == originals[r["id"]]["candidate_ids"][int(np.argmax(originals[r["id"]]["logits"]))]
               for r in paraphrases)
    report = {"status": "bounded_pilot_passed" if policy["acceptance_passed"] else "not_ready",
        "scope": PROTOCOL["scope"], "limitations": PROTOCOL["exclusions"],
        "synthetic_only": True, "no_real_repository_acceptance_yet": True,
        "checkpoint": str(checkpoint), "adapter_sha256":digest,
        "training":training, "selection":read_json(run/"selected.json"), "manifest":read_json(DATA/"manifest.json"),
        "policy":policy,"calibration":calibration_details,"metrics":metrics,"gates":gates,
        "paraphrase_consistency":same/len(paraphrases),"paraphrase_n":len(paraphrases),
        "reload_max_logit_error":reload_error, "random":random_metrics(combined),
        "public_transfer_manifest":read_json(TRANSFER_DIRECTORY/"manifest.json"),
        "notes":["32 authored training families, 8 completely held-out families. Instance counts are not counts of independent real bugs.",
                 "Missing-patch examples include an explicit review option. This does not prove general error detection.",
                 "Confidence thresholds and temperatures were chosen using calibration only. Weights were chosen using validation only.",
                 "Wilson bounds describe this fixed synthetic sample; they are not guarantees for user repositories.",
                 "The developer API checks original and reversed candidate order. It never executes or applies supplied code."]}
    write_json(output / "report.json",report)
    write_json(run / "evaluation.json",{"report":str((output/"report.json").relative_to(run))})
    write_report(report,run/"REPORT.md")
    print(json.dumps({"status":report["status"],"gates_passed":sum(g["passed"] for g in gates),"gates_total":len(gates),
                      "accuracy":{d:{"seen":m["seen_calibrated"]["accuracy"],"held_out":m["held_out_calibrated"]["accuracy"]} for d,m in metrics.items()}}),flush=True)
    return report


def write_report(report,path):
    lines=["# Python／前端開發決策模型", "", f"驗收狀態：`{report['status']}`。", "",
           "用途是選擇使用者提供的短程式修法。此模型不會直接產生完整程式，也不會修改檔案。", "",
           "所有資料都是本專案自製、經執行檢查的練習題。32 種題型參與訓練。另有 8 種題型完全保留。這些分數不能替代真實專案驗收。", "",
           "| 領域 | 已見題型的新題 | 未見題型 | Qwen 已見題型 | Qwen 未見題型 |", "|---|---:|---:|---:|---:|"]
    for domain,m in report["metrics"].items():
        lines.append(f"| {domain} | {m['seen_calibrated']['accuracy']:.1%} | {m['held_out_calibrated']['accuracy']:.1%} | {m['qwen_seen']['accuracy']:.1%} | {m['qwen_held_out']['accuracy']:.1%} |")
    lines += ["", "## 公開程式的額外檢查", "", "另用 Exercism 的 6 個公開函式檢查轉移能力。每個函式有 10 個候選組合，並非 60 個獨立問題。這些資料完全沒有用來訓練或選權重。", "",
              "| 領域 | SOM | 原始 Qwen |", "|---|---:|---:|"]
    for domain,m in report["metrics"].items():
        lines.append(f"| {domain} | {m['public_transfer']['accuracy']:.1%} | {m['qwen_public_transfer']['accuracy']:.1%} |")
    lines += ["", "此處的 JavaScript 只有基本函式，不代表 DOM 或 React 專案驗收。公開程式可能曾出現在基礎模型的訓練資料中。"]
    lines += ["", "## 固定驗收條件", "", "這些門檻在正式訓練前已寫入資料規格。沒有依測試分數降低門檻。", "",
              "| 領域 | 條件 | 實測 | 目標 | 通過 |", "|---|---|---:|---:|---|"]
    for g in report["gates"]:
        lines.append(f"| {g['domain']} | {g['gate']} | {g['actual']:.4f} | {g['operator']} {g['target']} | {'是' if g['passed'] else '否'} |")
    lines += ["", "## 需要人工檢查的情況", "", "沒有合適修法、交換候選順序後答案改變、信心不足，或整體驗收未通過時，API 都會回傳 `human_review`。", "",
              "| 領域 | 接受題數／測試題數 | 接受題誤判率 | 誤判率 95% 上界 | 兩次評分中位時間 |", "|---|---:|---:|---:|---:|"]
    for domain,m in report["metrics"].items():
        s=m["selective_seen"]
        risk="無样本" if s["error_rate"] is None else f"{s['error_rate']:.1%}"
        lines.append(f"| {domain} | {s['accepted']}/{s['n']} | {risk} | {s['error_wilson_upper_95']:.1%} | {m['paired_latency_ms']['p50']:.1f} ms |")
    lines += ["", "上界用 Wilson 方法計算，表示這批樣本的不確定性。它不是使用者專案的可靠度保證。", "",
              f"問題改寫後答案一致率：{report['paraphrase_consistency']:.1%}，共 {report['paraphrase_n']} 題。",
              f"重新載入的最大分數差：{report['reload_max_logit_error']:.8f}。", "",
              f"訓練處理 {report['training']['samples_seen']} 筆次，計時 {report['training']['elapsed_seconds']:.1f} 秒。",
              f"MLX 峰值 {report['training']['peak_mlx_gib']:.2f} GiB，程序 RSS 峰值 {report['training']['peak_process_rss_gib']:.2f} GiB。", "",
              "## 使用邊界", "", "Python 包含常見資料處理、例外及非同步載入。前端包含 JavaScript、DOM 操作與 React 風格的狀態更新函式。", "",
              "尚未驗證完整專案修復、CSS 畫面、TypeScript 型別檢查、React 元件生命週期或部署。使用前仍須檢查修法並跑自己的測試。", "",
              "完整機率誤差、每題分數、各題型結果及門檻選擇過程，見 evaluation.json 指向的報告。", ""]
    Path(path).write_text("\n".join(lines))
