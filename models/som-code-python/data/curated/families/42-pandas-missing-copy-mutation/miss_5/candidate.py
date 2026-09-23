"""Derive cohort features on a copy so the caller's DataFrame never changes."""

import numpy as np
from pandas import DataFrame


class CohortFeatureTransformer:
    """Score one cohort of a DataFrame without mutating the master table."""

    def __init__(self, cohort_col: str = "cohort", base_scale: float = 1.0) -> None:
        self.cohort_col = cohort_col
        self.base_scale = base_scale

    def transform_cohort(
        self,
        df: DataFrame,
        cohort_name: str,
        score_col: str = "score",
        scale: float | None = None,
    ) -> DataFrame:
        """Return the cohort's rows with normalized_score and risk_tier added."""
        if self.cohort_col not in df.columns:
            raise ValueError(f"Cohort column '{self.cohort_col}' not found")
        if score_col not in df.columns:
            raise ValueError(f"Score column '{score_col}' not found")
        if scale is not None and scale <= 0.0:
            raise ValueError("scale must be strictly positive (> 0.0)")

        effective_scale = self.base_scale if scale is None else scale
        in_cohort = df[self.cohort_col] == cohort_name
        if not in_cohort.any():
            raise ValueError(f"Cohort '{cohort_name}' not found in dataset")

        enriched = df
        enriched["normalized_score"] = enriched[score_col] * effective_scale
        enriched["risk_tier"] = np.where(
            enriched["normalized_score"] >= 50.0, "high", "standard"
        )
        return enriched[in_cohort]
