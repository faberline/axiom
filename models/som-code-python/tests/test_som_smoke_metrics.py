"""CPU-only checks for bounded SOM smoke accounting and snapshots."""

import pytest

from som.specialists.som_metrics import smoke_metric_breakdown
from som.specialists.som_train import _smoke_snapshot


def test_smoke_metric_breakdown_exposes_present_and_missing_counts():
    result = smoke_metric_breakdown(
        loss=0.75, correct=22, rows=32,
        present_correct=14, present_rows=16,
        missing_review_correct=8, missing_rows=16,
    )

    assert result == {
        "loss": 0.75,
        "accuracy": 22 / 32,
        "rows": 32,
        "correct": 22,
        "present_accuracy": 14 / 16,
        "present_rows": 16,
        "present_correct": 14,
        "missing_review_recall": 8 / 16,
        "missing_rows": 16,
        "missing_review_correct": 8,
    }


def test_smoke_snapshot_keeps_per_pass_history_and_locked_run_fields():
    metrics = smoke_metric_breakdown(
        loss=0.75, correct=22, rows=32,
        present_correct=14, present_rows=16,
        missing_review_correct=8, missing_rows=16,
    )
    result = _smoke_snapshot(
        "python", 32, metrics, metrics, passes=2, updates=8,
        reduction=0.25,
        history=[{"pass": 1, "updates": 4, "metrics": metrics, "loss_reduction": 0.1}],
    )

    assert result["status"] == "running"
    assert result["passed"] is False
    assert result["pass_history"][0]["pass"] == 1
    assert result["before"]["present_rows"] == 16
    assert result["after"]["missing_review_correct"] == 8


def test_smoke_metric_breakdown_rejects_inconsistent_overall_count():
    with pytest.raises(ValueError, match="overall correct count"):
        smoke_metric_breakdown(
            loss=0.75, correct=21, rows=32,
            present_correct=14, present_rows=16,
            missing_review_correct=8, missing_rows=16,
        )
