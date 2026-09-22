"""Evaluate frozen browser mechanisms after the primary acceptance report."""
import gc
import json
from pathlib import Path

import mlx.core as mx
import numpy as np

from ..metrics import summarize
from ..paths import ROOT,read_json,write_json,resolve_checkpoint,sha256
from .ranker_browser import DATA,rows
from .ranker_model import load_ranker,Scorer,restore
from .next_train import RUN
from .next_train import verify_protocol
from .coder_records import collect
from .policy import selective_metrics
from .next_holdout import rows as public_rows

UNSEEN_QUESTIONS=("After reviewing the bug report, which proposed repair would you approve? Request human review when all proposals fail.",
                  "Pick a replacement that fulfills the specification and handles the edge cases. If no replacement works, choose human review.")


def evaluate(run=RUN):
    run=Path(run); verify_protocol(run)
    report=read_json(run/read_json(run/"evaluation.json")["report"])
    checkpoint=resolve_checkpoint(run); digest=sha256(checkpoint/"adapter.safetensors")
    if digest!=report["adapter_sha256"]:
        raise ValueError("Main report uses a different checkpoint.")
    output=run/"browser-evaluation"/digest[:16]; output.mkdir(parents=True,exist_ok=True)
    provenance={"adapter_sha256":digest,"manifest_sha256":sha256(DATA/"manifest.json"),
                "main_report_sha256":sha256(run/read_json(run/"evaluation.json")["report"]),
                "code_sha256":sha256(Path(__file__))}
    if (output/"provenance.json").exists() and read_json(output/"provenance.json")!=provenance:
        raise ValueError("Browser evaluation inputs changed.")
    write_json(output/"provenance.json",provenance)
    values=rows(); model,tokenizer=load_ranker(checkpoint); scorer=Scorer(model,tokenizer)
    final=collect(values,scorer.paired,output/"trained.json")
    fresh=public_rows()
    rewritten=[{**r,"question":UNSEEN_QUESTIONS[i%2]} for d in ("python","frontend")
               for i,r in enumerate([r for r in fresh if r["task"]==d][:16])]
    paraphrases=collect(rewritten,scorer.paired,output/"question-paraphrases.json")
    original=read_json(run/"evaluation"/digest[:16]/"public-final.json")
    indexed={r["id"]:r for r in original}
    reworded={}
    for domain in ("python","frontend"):
        records=[r for r in paraphrases if r["task"]==domain]
        temperature=report["policy"]["domains"][domain]["temperature"]
        agreement=float(np.mean([r["candidate_ids"][int(np.argmax(r["logits"]))]==indexed[r["id"]]["candidate_ids"][int(np.argmax(indexed[r["id"]]["logits"]))] for r in records]))
        reworded[domain]={"n":len(records),"choice_agreement":agreement,"metrics":summarize(records,temperature)}
    restore(model,run/"initial"); scorer=Scorer(model,tokenizer)
    initial=collect(values,scorer.paired,output/"v4.json")
    t=report["policy"]["domains"]["frontend"]["temperature"]
    threshold=report["policy"]["domains"]["frontend"]["threshold"]
    result={**provenance,"manifest":read_json(DATA/"manifest.json"),
        "trained":summarize(final,t),"base":summarize(initial),"question_paraphrases":reworded,
        "selective":selective_metrics(final,t,threshold),
        "families":{f:{"trained":summarize([r for r in final if r["family"]==f],t),
                       "base":summarize([r for r in initial if r["family"]==f])} for f in sorted({r["family"] for r in final})},
        "limits":"Previously evaluated v4 diagnostics; regression only. Six mechanisms, eight related examples each. jsdom is not a real browser. The v4 missing-answer errors informed v5 design. These cases were never used for gradients, checkpoint selection or calibration."}
    write_json(output/"report.json",result); write_json(run/"browser-evaluation.json",{"report":str((output/"report.json").relative_to(run))})
    lines=["# v5 前端行為檢查","",f"48 個例子來自 6 種自製題型。正確率：v4 {result['base']['accuracy']:.1%} → 訓練後 {result['trained']['accuracy']:.1%}。",
           "這批是 v4 舊題。舊錯誤影響了 v5 設計，所以結果只算回歸檢查。題目未參與梯度更新、選權重或校準。","",
           "| 題型 | v4 | 訓練後 |","|---|---:|---:|"]
    for f,m in result["families"].items():
        lines.append(f"| {f} | {m['base']['accuracy']:.1%} | {m['trained']['accuracy']:.1%} |")
    lines.extend(["","另以訓練未用過的兩種英文問法，各檢查 16 筆 Python 與 JavaScript 公開題。",
                  f"改寫後答案一致：Python {reworded['python']['choice_agreement']:.1%}，JavaScript {reworded['frontend']['choice_agreement']:.1%}。",
                  "這些是少量補充檢查，不會改變權重或驗收門檻。"])
    lines.extend(["","每種題型的例子相關，不能視為 48 個獨立問題。","測試在 Node/jsdom 執行，沒有真實瀏覽器、CSS 視覺或完整 React 應用程式測試。",
                  "主要驗收結果與 API 的人工檢查規則保持不變。",""])
    (run/"BROWSER.md").write_text("\n".join(lines))
    print(json.dumps({"browser_accuracy":result["trained"]["accuracy"],"base":result["base"]["accuracy"]}),flush=True)
    del scorer,model; gc.collect(); mx.clear_cache()
    return result


if __name__=="__main__":
    import argparse
    parser=argparse.ArgumentParser(); parser.add_argument("--run",type=Path,default=RUN)
    evaluate(parser.parse_args().run)
