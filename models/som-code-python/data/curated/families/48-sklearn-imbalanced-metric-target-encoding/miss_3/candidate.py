"""Score binary classifiers on imbalanced data without trusting raw accuracy."""

import numpy as np
from sklearn.metrics import (
    accuracy_score,
    balanced_accuracy_score,
    precision_recall_fscore_support,
    roc_auc_score,
)


class ImbalancedClassificationEvaluator:
    """Report balanced metrics and withhold raw accuracy under heavy imbalance."""

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
            raise ValueError(
                f"imbalance_threshold must be > 1.0, got {imbalance_threshold}"
            )

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
        if not 0.0 <= threshold <= 1.0:
            raise ValueError(f"threshold must be between 0.0 and 1.0, got {threshold}")
        return np.where(prob_arr >= threshold, 1, 0)

    def evaluate(
        self,
        y_true: np.ndarray,
        y_pred: np.ndarray | None = None,
        y_prob: np.ndarray | None = None,
    ) -> dict[str, float]:
        """Return balanced metrics for pos_label, plus roc_auc when y_prob is given."""
        if y_pred is None:
            if y_prob is None:
                raise ValueError("Either y_pred or y_prob must be provided")
            y_pred = self.threshold_probabilities(y_prob)

        y_t = np.asarray(y_true, dtype=np.int64)
        y_p = np.asarray(y_pred, dtype=np.int64)
        if len(y_t) != len(y_p):
            raise ValueError(
                f"y_true and y_pred lengths must match: {len(y_t)} vs {len(y_p)}"
            )
        if not set(np.unique(y_t)) | set(np.unique(y_p)) <= {0, 1}:
            raise ValueError("Target labels and predictions must be binary in {0, 1}")

        imbalance_ratio = self._imbalance_ratio(y_t)
        precision, recall, f1, _ = precision_recall_fscore_support(
            y_t, y_p, pos_label=self.pos_label, average="binary", zero_division=0.0
        )
        metrics = {
            "balanced_accuracy": float(balanced_accuracy_score(y_t, y_p)),
            "precision": float(precision),
            "recall": float(recall),
            "f1": float(f1),
            "imbalance_ratio": imbalance_ratio,
        }

        if y_prob is not None:
            prob_arr = np.asarray(y_prob, dtype=np.float64)
            if len(prob_arr) != len(y_t):
                raise ValueError("y_prob and y_true lengths must match")
            auc = float(roc_auc_score(y_t, prob_arr))
            metrics["roc_auc"] = 1.0 - auc if self.pos_label == 0 else auc

        if imbalance_ratio < self.imbalance_threshold:
            metrics["accuracy"] = float(accuracy_score(y_t, y_p))

        return metrics

    def _imbalance_ratio(self, y_t: np.ndarray) -> float:
        """Return the majority-to-minority ratio, refusing a too-small minority."""
        n_1 = int(np.count_nonzero(y_t))
        minority, majority = sorted((len(y_t) - n_1, n_1))
        if minority <= self.min_minority:
            raise ValueError(
                f"Minority class count {minority} is less than required minimum "
                f"{self.min_minority}"
            )
        return majority / minority
