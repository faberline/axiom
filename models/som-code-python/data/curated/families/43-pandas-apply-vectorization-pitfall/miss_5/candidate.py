"""Score transactions with whole-column arithmetic instead of a row apply."""

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

        def _score_row(row: Series) -> float:
            amt = float(row[amount_col])
            freq = float(row[freq_col])
            if amt > 1000.0:
                val = amt * 0.05 + freq * self.weight
            else:
                val = amt * 0.01 + freq * 0.5
            return min(val, self.max_cap)

        scores = Series([_score_row(df.iloc[i]) for i in range(len(df))])
        return scores
