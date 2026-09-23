"""Update DataFrame rows through one .loc call instead of chained indexing."""

from typing import Any

from pandas import DataFrame


class ConditionalRecordUpdater:
    """Set a column on every row whose filter column meets a threshold."""

    def __init__(self, default_col: str = "status") -> None:
        self.default_col = default_col

    def update_records(  # noqa: PLR0913  # pylint: disable=too-many-arguments
        self,
        df: DataFrame,
        filter_col: str,
        threshold: float,
        update_col: str,
        new_value: Any,
        *,
        inplace: bool = True,
    ) -> DataFrame:
        """Set update_col to new_value where filter_col is at least threshold."""
        if not isinstance(df, DataFrame):
            raise TypeError("df must be an instance of DataFrame")
        if filter_col not in df.columns:
            raise ValueError(f"filter_col '{filter_col}' not found in columns")
        if update_col not in df.columns:
            raise ValueError(f"update_col '{update_col}' not found in columns")
        if threshold < 0.0:
            raise ValueError("threshold must be non-negative (>= 0.0)")

        target_df = df if inplace else df.copy()
        mask = target_df[filter_col] >= threshold
        target_df.loc[mask, update_col] = new_value
        return target_df
