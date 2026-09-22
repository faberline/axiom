import numpy as np
from scipy.optimize import minimize_scalar


def probabilities(logits, temperature=1.0):
    z = np.asarray(logits, dtype=np.float64) / temperature
    z -= z.max()
    e = np.exp(z)
    return e / e.sum()


def nll(logits, gold, temperature=1.0):
    z = np.asarray(logits, dtype=np.float64) / temperature
    maximum = z.max()
    return float(maximum + np.log(np.exp(z - maximum).sum()) - z[gold])


def fit_temperature(records):
    def objective(log_temperature):
        return float(np.mean([nll(r["logits"], r["gold_index"], np.exp(log_temperature)) for r in records]))
    result = minimize_scalar(objective, bounds=(-4, 4), method="bounded")
    # Identity is always an available calibration candidate.
    log_t = float(result.x) if result.success and result.fun < objective(0.0) else 0.0
    return {"temperature": float(np.exp(log_t)), "calibration_n": len(records),
            "nll_before": objective(0.0), "nll_after": objective(log_t)}


def summarize(records, temperature=1.0):
    ps = [probabilities(r["logits"], temperature) for r in records]
    correct = np.array([int(p.argmax()) == r["gold_index"] for r, p in zip(records, ps)])
    confidence = np.array([float(p.max()) for p in ps])
    brier = [float(((p - np.eye(len(p))[r["gold_index"]]) ** 2).sum()) for r, p in zip(records, ps)]
    reliability = []
    ece = 0.0
    for i in range(10):
        mask = (confidence >= i / 10) & ((confidence < (i + 1) / 10) if i < 9 else (confidence <= 1.0))
        count = int(mask.sum())
        if count:
            accuracy, conf = float(correct[mask].mean()), float(confidence[mask].mean())
            ece += count / len(records) * abs(accuracy - conf)
            reliability.append({"lower": i / 10, "n": count, "accuracy": accuracy, "confidence": conf})
    risk = []
    for threshold in (0.0, 0.5, 0.7, 0.8, 0.9, 0.95):
        mask = confidence >= threshold
        n = int(mask.sum())
        risk.append({"threshold": threshold, "accepted": n, "coverage": n / len(records),
                     "error": float(1 - correct[mask].mean()) if n else None})
    latencies = [r["seconds"] * 1000 for r in records]
    return {"n": len(records), "accuracy": float(correct.mean()),
            "random_accuracy": float(np.mean([1 / len(p) for p in ps])),
            "nll": float(np.mean([nll(r["logits"], r["gold_index"], temperature) for r in records])),
            "brier": float(np.mean(brier)), "ece_10_bins": ece,
            "reliability": reliability, "risk_coverage": risk,
            "latency_ms": {f"p{q}": float(np.percentile(latencies, q)) for q in (50, 95, 99)}}


def grouped_metrics(records, temperature=1.0):
    return {task: summarize([r for r in records if r["task"] == task], temperature)
            for task in sorted({r["task"] for r in records})}
