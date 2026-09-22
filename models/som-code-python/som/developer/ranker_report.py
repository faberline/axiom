"""Render saved results without loading a GPU model."""
from pathlib import Path


def write_report(report,path):
    pct=lambda x:f"{100*x:.1f}%"
    metrics=report["metrics"]; training=report["training"]
    records=Path(report["checkpoint"]).resolve().parents[1]/"evaluation"/report["adapter_sha256"][:16]
    passed=sum(g["passed"] for g in report["gates"])
    lines=["# Python／JavaScript 第四輪實測", "",
           f"狀態：`{report['status']}`。固定驗收條件通過 {passed}/{len(report['gates'])} 項。", "",
           "此模型從你提供的候選修法中選擇。它不產生完整程式，也不修改或執行使用者的程式。", "",
           "若總驗收未通過，API 的所有輸出都要求人工檢查。下表的篩選數量只是假設採用信心規則時的離線結果。", "",
           "| 項目 | Python | JavaScript |", "|---|---:|---:|"]
    for title,key,field in (("已見題型的新例子","seen","accuracy"),("公開新問題","public_final","accuracy"),
                            ("原模型：公開新問題","base_public_final","accuracy"),("上一輪公開題回歸測試","previous_public_regression","accuracy"),("隨機選擇","public_final","random_accuracy"),
                            ("候選換序答案一致","selective_public_final","order_consistency"),
                            ("缺少正解時選人工檢查","selective_public_final","missing_fix_review_recall"),
                            ("改寫問題後答案一致","question_paraphrase","choice_agreement")):
        lines.append(f"| {title} | {pct(metrics['python'][key][field])} | {pct(metrics['frontend'][key][field])} |")
    lines += ["", "公開新測試有 40 個先前未評分的問題，各有兩種語言及三個候選版本。總共 240 題，但不能算成 240 個獨立問題。",
              "公開資料可能出現在基礎模型的預訓練中。這次微調、選權重及校準均未使用它。",
              "JavaScript 測試只驗證函式邏輯。它不能證明 React、DOM、CSS 或整個前端專案可用。", "",
              "| 機率與篩選 | Python | JavaScript |","|---|---:|---:|"]
    for domain in ("python","frontend"):
        m=metrics[domain]; s=m["selective_public_final"]
        m["_display"]={"accepted":f"{s['accepted']}/{s['n']}","errors":str(s["errors"]),
                       "upper":pct(m["accepted_error_gate_upper"]) if s["accepted"] else "無可接受樣本",
                       "brier":f"{m['public_final_raw']['brier']:.3f} → {m['public_final']['brier']:.3f}",
                       "ece":f"{pct(m['public_final_raw']['ece_10_bins'])} → {pct(m['public_final']['ece_10_bins'])}",
                       "latency":f"{m['paired_latency_ms']['p50']/1000:.2f} / {m['paired_latency_ms']['p95']/1000:.2f} 秒"}
    for label,key in (("信心規則留下的候選","accepted"),("其中答錯","errors"),("錯誤率上界","upper"),
                      ("Brier：校準前 → 後","brier"),("ECE：校準前 → 後","ece"),("API 評分中位／95百分位","latency")):
        lines.append(f"| {label} | {metrics['python']['_display'][key]} | {metrics['frontend']['_display'][key]} |")
    for m in metrics.values():
        del m["_display"]
    lines += ["", "候選換序一致是架構保證，不代表模型學會更多程式知識。另有 32 題以未快取的推論檢查換序。API 反序呼叫使用相同候選的已算分數，因此時間不是兩次 GPU 推論。",
              "Brier 衡量整組機率的誤差。ECE 衡量信心與實際正確率的差距。兩者越低越好。",
              "錯誤率上界取兩種估計中較大者：逐題 Wilson 上界，以及按問題分組重抽樣的上界。",
              "機率校準只使用另外 316 題。它不會改變答案排名。", "",
              "下表只看單次答案的信心，包含人工檢查選項。它未套用雙向一致與總驗收規則，不能當作 API 推薦數。", "",
              "| 信心門檻 | Python：題數／誤判率 | JavaScript：題數／誤判率 |","|---|---:|---:|"]
    for i,trial in enumerate(metrics["python"]["public_final"]["risk_coverage"]):
        values=[]
        for domain in ("python","frontend"):
            r=metrics[domain]["public_final"]["risk_coverage"][i]
            values.append(f"{r['accepted']}／{pct(r['error']) if r['error'] is not None else '無樣本'}")
        lines.append(f"| {trial['threshold']:.2f} | {values[0]} | {values[1]} |")
    lines += ["",
              f"訓練讀取 {training['samples_seen']} 筆，共 {training['updates']} 次更新。每個候選各自評分，並使用同一組參數。",
              f"訓練流程累計 {training['elapsed_seconds']/60:.1f} 分鐘，包含快取與驗證。{'累計時間也包含資料修正前的快取工作。' if training['protocol'].get('preparation_carry_sha256') else ''}MLX 峰值 {training['peak_mlx_gib']:.2f} GiB。",
              "只更新最後一個完整注意力層的 query/value LoRA，共 114,688 個參數。前 31 層保持凍結。",
              f"權重由驗證資料選定：`{report['selection']['kind']}`。重載最大分數差為 {report['reload_max_logit_error']:.8g}。", "",
              "| 領域 | 驗收條件 | 實際 | 門檻 | 結果 |","|---|---|---:|---:|---|"]
    for gate in report["gates"]:
        lines.append(f"| {gate['domain']} | {gate['gate']} | {gate['actual']:.4f} | {gate['mode']} {gate['target']} | {'通過' if gate['passed'] else '未通過'} |")
    lines += ["",f"檢查點：`{report['checkpoint']}`",f"權重 SHA256：`{report['adapter_sha256']}`", "",
              f"[公開題逐題分數]({records/'public-final.json'})、[原模型逐題分數]({records/'base-public-final.json'})與[完整 JSON 報告]({records/'report.json'})均保存在本機。",
              "上面的 API 評分時間混合已見題型與公開題。較長程式的處理時間通常較長。",
              "操作方式見專案根目錄的 RANKER.md。", ""]
    path.write_text("\n".join(lines))
