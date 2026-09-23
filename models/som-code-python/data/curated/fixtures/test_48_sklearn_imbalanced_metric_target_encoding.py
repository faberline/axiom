"""Oracle test suite for Family 48: Imbalanced Classification Metric Evaluator."""

import numpy as np
import pytest
from candidate import ImbalancedClassificationEvaluator


def test_dummy_all_zero_model_has_zero_recall_and_f1():
    # 90 majority class (0) and 10 minority class (1) -> 9:1 imbalance
    y_true = np.array([0] * 90 + [1] * 10)
    y_pred = np.array([0] * 100)  # Dummy classifier predicting only majority class

    evaluator = ImbalancedClassificationEvaluator(pos_label=1, min_minority=5, imbalance_threshold=5.0)
    metrics = evaluator.evaluate(y_true, y_pred)

    # For minority class (1), recall and precision must be 0.0
    assert metrics["recall"] == 0.0
    assert metrics["precision"] == 0.0
    assert metrics["f1"] == 0.0
    # Balanced accuracy: (1.0 + 0.0) / 2 = 0.5
    assert metrics["balanced_accuracy"] == 0.5
    # Raw accuracy must be suppressed because 9.0 >= imbalance_threshold (5.0)
    assert "accuracy" not in metrics


def test_default_pos_label_evaluates_minority_class():
    evaluator = ImbalancedClassificationEvaluator()
    # Canonical positive class in anomaly/fraud detection is 1
    assert evaluator.pos_label == 1, f"Expected default pos_label=1, got {evaluator.pos_label}"


def test_rejects_non_binary_target_labels():
    evaluator = ImbalancedClassificationEvaluator(min_minority=2)
    y_multiclass = np.array([0, 1, 2, 0, 1, 0, 1, 0, 1, 0])
    y_pred = np.array([0, 1, 0, 0, 1, 0, 1, 0, 1, 0])

    with pytest.raises(ValueError, match="binary in {0, 1}"):
        evaluator.evaluate(y_multiclass, y_pred)


def test_boundary_exact_min_minority_accepted():
    # Exactly 5 minority samples matches min_minority=5
    y_true = np.array([0] * 95 + [1] * 5)
    y_pred = np.array([0] * 95 + [1] * 5)

    evaluator = ImbalancedClassificationEvaluator(pos_label=1, min_minority=5, imbalance_threshold=5.0)
    metrics = evaluator.evaluate(y_true, y_pred)
    assert metrics["balanced_accuracy"] == 1.0
    assert metrics["recall"] == 1.0


def test_probability_threshold_positive_branch():
    evaluator = ImbalancedClassificationEvaluator()
    probs = np.array([0.9, 0.75, 0.2, 0.05])
    preds = evaluator.threshold_probabilities(probs, threshold=0.5)
    # High probabilities (>= 0.5) must map to positive class 1
    np.testing.assert_array_equal(preds, np.array([1, 1, 0, 0]))


def test_refuses_raw_accuracy_on_imbalanced_data():
    y_true = np.array([0] * 95 + [1] * 5)
    y_pred = np.array([0] * 100)

    evaluator = ImbalancedClassificationEvaluator(imbalance_threshold=5.0)
    metrics = evaluator.evaluate(y_true, y_pred)

    assert "balanced_accuracy" in metrics, "balanced_accuracy must be reported for imbalanced data"
    assert "accuracy" not in metrics, "Misleading raw accuracy must be omitted when imbalance exceeds threshold"


def test_roc_auc_computation_when_prob_provided():
    y_true = np.array([0] * 50 + [1] * 10)
    # Give high probabilities to positives and low to negatives
    probs = np.concatenate([np.linspace(0.01, 0.49, 50), np.linspace(0.51, 0.99, 10)])

    evaluator = ImbalancedClassificationEvaluator(pos_label=1, min_minority=5, imbalance_threshold=3.0)
    metrics = evaluator.evaluate(y_true, y_prob=probs)

    assert "roc_auc" in metrics
    assert metrics["roc_auc"] > 0.95
