"""Imbalanced classification evaluator computing balanced metrics and suppressing misleading raw accuracy."""

from __future__ import annotations

import numpy as np


class ImbalancedClassificationEvaluator:
    """Evaluates binary classification performance under severe class imbalance."""

    def __init__(
        self,
        pos_label: int = 1,
        min_minority: int = 5,
        imbalance_threshold: float = 5.0,
    ) -> None:
        if pos_label not in (0, 1):
            raise ValueError(f"pos_label must be 0 or 1, got {pos_label}")
        if min_minority < 1:
            raise ValueError(f"min_minority must be >= 1, got {min_minority}")
        if imbalance_threshold <= 1.0:
            raise ValueError(f"imbalance_threshold must be > 1.0, got {imbalance_threshold}")

        self.pos_label = pos_label
        self.min_minority = min_minority
        self.imbalance_threshold = imbalance_threshold

    def threshold_probabilities(
        self,
        y_prob: np.ndarray,
        threshold: float = 0.5,
    ) -> np.ndarray:
        """Map predicted positive-class probabilities to binary labels."""
        prob_arr = np.asarray(y_prob, dtype=np.float64)
        if not (0.0 <= threshold <= 1.0):
            raise ValueError(f"threshold must be between 0.0 and 1.0, got {threshold}")
        return np.where(prob_arr >= threshold, 1, 0)

    def evaluate(
        self,
        y_true: np.ndarray,
        y_pred: np.ndarray | None = None,
        y_prob: np.ndarray | None = None,
    ) -> dict[str, float]:
        """Compute balanced classification metrics for imbalanced datasets."""
        if y_pred is None:
            if y_prob is None:
                raise ValueError("Either y_pred or y_prob must be provided")
            y_pred = self.threshold_probabilities(y_prob)

        y_t = np.asarray(y_true, dtype=np.int64)
        y_p = np.asarray(y_pred, dtype=np.int64)

        if len(y_t) != len(y_p):
            raise ValueError(f"y_true and y_pred lengths must match: {len(y_t)} vs {len(y_p)}")

        n_0 = int(np.sum(y_t == 0))
        n_1 = int(np.sum(y_t == 1))
        minority_count = min(n_0, n_1)
        majority_count = max(n_0, n_1)

        if minority_count < self.min_minority:
            raise ValueError(
                f"Minority class count {minority_count} is less than required minimum {self.min_minority}"
            )

        imbalance_ratio = float(majority_count) / max(1, minority_count)

        rec_0 = float(np.sum((y_t == 0) & (y_p == 0)) / n_0) if n_0 > 0 else 0.0
        rec_1 = float(np.sum((y_t == 1) & (y_p == 1)) / n_1) if n_1 > 0 else 0.0
        balanced_acc = (rec_0 + rec_1) / 2.0

        # Compute precision and recall for pos_label
        target_val = self.pos_label
        tp = float(np.sum((y_t == target_val) & (y_p == target_val)))
        fp = float(np.sum((y_t != target_val) & (y_p == target_val)))
        fn = float(np.sum((y_t == target_val) & (y_p != target_val)))

        precision = tp / (tp + fp) if (tp + fp) > 0.0 else 0.0
        recall = tp / (tp + fn) if (tp + fn) > 0.0 else 0.0
        f1 = (2.0 * precision * recall / (precision + recall)) if (precision + recall) > 0.0 else 0.0

        metrics: dict[str, float] = {
            "balanced_accuracy": float(balanced_acc),
            "precision": float(precision),
            "recall": float(recall),
            "f1": float(f1),
            "imbalance_ratio": float(imbalance_ratio),
        }

        if y_prob is not None:
            prob_arr = np.asarray(y_prob, dtype=np.float64)
            if len(prob_arr) != len(y_t):
                raise ValueError("y_prob and y_true lengths must match")
            pos_mask = (y_t == 1)
            neg_mask = (y_t == 0)
            n_pos = int(np.sum(pos_mask))
            n_neg = int(np.sum(neg_mask))
            if n_pos > 0 and n_neg > 0:
                ranks = np.argsort(np.argsort(prob_arr)) + 1
                pos_rank_sum = float(np.sum(ranks[pos_mask]))
                auc = (pos_rank_sum - n_pos * (n_pos + 1) / 2.0) / (n_pos * n_neg)
                if self.pos_label == 0:
                    auc = 1.0 - auc
                metrics["roc_auc"] = float(auc)

        if imbalance_ratio < self.imbalance_threshold:
            metrics["accuracy"] = float(np.mean(y_t == y_p))

        return metrics
