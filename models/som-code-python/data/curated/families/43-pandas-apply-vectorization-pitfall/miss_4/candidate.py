"""Score transactions with whole-column arithmetic instead of a row apply."""

import numpy as np
from pandas import DataFrame, Series


class TransactionVectorizedScorer:
    """High-throughput transaction risk scorer using vectorized operations."""

    def __init__(self, weight: float = 1.5, max_cap: float = 100.0) -> None:
        if weight <= 0.0:
            raise ValueError("weight must be strictly positive (> 0.0)")
        if max_cap <= 0.0:
            raise ValueError("max_cap must be strictly positive (> 0.0)")
        self.weight = weight
        self.max_cap = max_cap

    def score_transactions(
        self,
        df: DataFrame,
        amount_col: str = "amount",
        freq_col: str = "frequency",
    ) -> Series:
        """Return one risk score per row, capped at max_cap, on the frame's index."""
        if df.empty:
            raise ValueError("DataFrame must not be empty")
        if amount_col not in df.columns:
            raise ValueError(f"Column '{amount_col}' not found")
        if freq_col not in df.columns:
            raise ValueError(f"Column '{freq_col}' not found")

        high_mask = df[amount_col] > 1000.0
        base = np.where(
            high_mask,
            df[amount_col] * 0.01 + df[freq_col] * 0.5,
            df[amount_col] * 0.05 + df[freq_col] * self.weight,
        )
        return Series(base, index=df.index).clip(upper=self.max_cap)
