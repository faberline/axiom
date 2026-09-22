"""Render a saved coder report without loading MLX or a model."""
from pathlib import Path


def write_report(report,path):
    lines=["# Coder v2 開發決策模型", "", f"狀態：`{report['status']}`。", "",
        "此版本用 Qwen2.5-Coder-7B 的原有字母評分能力，並加入小幅 QLoRA 微調。QLoRA 是在量化權重上訓練少量附加參數。", "",
        "用途是評分 2–7 個候選修法，另保留人工檢查選項。模型不會執行輸入程式或修改檔案。", "",
        "| 領域 | 已見題型新題 | 全新題型 | 原始 7B 的已見題型 | 原始 7B 的全新題型 | 隨機選擇全新題型 |", "|---|---:|---:|---:|---:|---:|"]
    for d,m in report["metrics"].items():
        lines.append(f"| {d} | {m['seen']['accuracy']:.1%} | {m['fresh_final']['accuracy']:.1%} | {m['base_seen']['accuracy']:.1%} | {m['base_fresh_final']['accuracy']:.1%} | {m['fresh_final']['random_accuracy']:.1%} |")
    lines += ["", "全新題型共有 12 種、240 個變體。它們沒有參與訓練、校準或權重選擇。這不是 240 個獨立真實專案問題。", "",
        "## 驗收條件", "", "| 領域 | 條件 | 實測 | 目標 | 通過 |", "|---|---|---:|---:|---|"]
    for g in report["gates"]:
        operator=">=" if g["mode"]=="min" else "<="
        lines.append(f"| {g['domain']} | {g['gate']} | {g['actual']:.4f} | {operator} {g['target']} | {'是' if g['passed'] else '否'} |")
    lines += ["", "## 建議模式", "", "信心門檻用獨立校準集選定。候選正序和反序的答案必須相同。全部驗收條件通過後，API 才會開放建議模式。", "",
        "下表是只套用信心與換序條件的離線統計。本輪未通過整體驗收，所以 API 實際上全部要求人工檢查。", "",
        "| 領域 | 接受的新題 | 接受題誤判率 | 誤判率 95% 上界 | 雙次評分中位時間 |", "|---|---:|---:|---:|---:|"]
    for d,m in report["metrics"].items():
        s=m["selective_fresh_final"]; error="無樣本" if s["error_rate"] is None else f"{s['error_rate']:.1%}"
        lines.append(f"| {d} | {s['accepted']}/{s['n']} | {error} | {s['error_wilson_upper_95']:.1%} | {m['paired_latency_ms']['p50']:.0f} ms |")
    lines += ["", "耗時排除模型載入與首次暖機。命令列首次啟動還需要載入權重。"]
    lines += ["", "## 新題型的機率誤差", "", "Brier 分數衡量整組候選機率的誤差。數值越低越好。ECE 是信心與實際答對率的平均差距，此處使用 10 組信心區間。", "",
              "| 領域 | Brier 校準前 | Brier 校準後 | ECE 校準前 | ECE 校準後 |", "|---|---:|---:|---:|---:|"]
    for d,m in report["metrics"].items():
        raw=m["fresh_final_raw"]; calibrated=m["fresh_final"]
        lines.append(f"| {d} | {raw['brier']:.3f} | {calibrated['brier']:.3f} | {raw['ece_10_bins']:.1%} | {calibrated['ece_10_bins']:.1%} |")
    lines += ["", "在校準集上調整機率，不能保證新題型的機率也準確。", "",
              "## 各新題型", "", "| 領域 | 題型 | 正確率 |", "|---|---|---:|"]
    for d,m in report["metrics"].items():
        for family,values in m["fresh_families"].items():
            lines.append(f"| {d} | {family} | {values['accuracy']:.1%} |")
    lines += ["", "## 補充診斷", "", "| 領域 | 改寫問題後答案一致率 | 公開函式微調後正確率 | 公開函式原模型正確率 |", "|---|---:|---:|---:|"]
    for d,m in report["metrics"].items():
        lines.append(f"| {d} | {m['question_paraphrase']['choice_agreement']:.1%} | {m['public_diagnostic']['accuracy']:.1%} | {m['base_public_diagnostic']['accuracy']:.1%} |")
    lines += ["", "改寫問題每個領域測 16 題。公開函式每個領域只有 3 個函式，各有 10 個候選順序變體。它們是先前已看過的診斷資料。"]
    lines += ["", "同一題型的變體彼此相關。Wilson 上界將它們當作獨立樣本，可能低估不確定性，不能作為正式專案的可靠度保證。", "",
        f"選用權重：`{report['selection']['kind']}`。權重選擇只比較驗證集 NLL。",
        f"本次微調 {report['training']['samples_seen']} 筆，訓練計時 {report['training']['elapsed_seconds']:.1f} 秒。",
        f"訓練 MLX 峰值 {report['training']['peak_mlx_gib']:.2f} GiB。重新載入最大分數差 {report['reload_max_logit_error']:.8f}。", "",
        "## 限制", "", "資料仍以自製、可執行檢查的短程式題為主。尚未驗收完整專案、自由產生程式、React 應用程式生命週期或部署。", "",
        "前一個 0.6B 原型未通過新題型驗收。它保留為失敗對照，沒有用高練習分數替代泛化證據。", "",
        "完整機率誤差與每題分數見 evaluation.json 指向的報告。", ""]
    Path(path).write_text("\n".join(lines))
