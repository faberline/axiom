"""Family 42: pandas missing copy mutation.

Enforces explicit .copy() on sliced subsets, validates parameters and cohort presence.
"""
from __future__ import annotations

from typing import Any, Dict, List, Optional
import numpy as np


class Series:
    """Lightweight 1D labeled array simulating pandas.Series."""

    def __init__(self, data: Any, index: Optional[List[Any]] = None, name: Optional[str] = None) -> None:
        if isinstance(data, Series):
            self._data = np.array(data._data, copy=True)
            self.index = list(data.index) if index is None else list(index)
        elif isinstance(data, np.ndarray):
            self._data = np.array(data, copy=True)
            self.index = list(range(len(self._data))) if index is None else list(index)
        elif isinstance(data, (list, tuple)):
            self._data = np.array(data)
            self.index = list(range(len(self._data))) if index is None else list(index)
        else:
            self._data = np.array([data])
            self.index = [0] if index is None else list(index)
        self.name = name

    @property
    def values(self) -> np.ndarray:
        return self._data

    def __len__(self) -> int:
        return len(self._data)

    def __array__(self, dtype: Any = None) -> np.ndarray:
        return np.asarray(self._data, dtype=dtype)

    def __getitem__(self, key: Any) -> Any:
        if isinstance(key, (list, np.ndarray, Series)):
            mask = np.asarray(key, dtype=bool)
            sub_data = self._data[mask]
            sub_index = [idx for idx, m in zip(self.index, mask) if m]
            return Series(sub_data, index=sub_index, name=self.name)
        if key in self.index:
            idx = self.index.index(key)
            return self._data[idx]
        if isinstance(key, (int, np.integer)) and 0 <= key < len(self._data):
            return self._data[key]
        raise KeyError(key)

    def __setitem__(self, key: Any, value: Any) -> None:
        if isinstance(key, (list, np.ndarray, Series)):
            mask = np.asarray(key, dtype=bool)
            self._data[mask] = value
        elif key in self.index:
            idx = self.index.index(key)
            self._data[idx] = value
        elif isinstance(key, (int, np.integer)) and 0 <= key < len(self._data):
            self._data[key] = value
        else:
            raise KeyError(key)

    def __ge__(self, other: Any) -> Series:
        other_val = other._data if isinstance(other, Series) else other
        return Series(self._data >= other_val, index=self.index)

    def __gt__(self, other: Any) -> Series:
        other_val = other._data if isinstance(other, Series) else other
        return Series(self._data > other_val, index=self.index)

    def __le__(self, other: Any) -> Series:
        other_val = other._data if isinstance(other, Series) else other
        return Series(self._data <= other_val, index=self.index)

    def __lt__(self, other: Any) -> Series:
        other_val = other._data if isinstance(other, Series) else other
        return Series(self._data < other_val, index=self.index)

    def __eq__(self, other: Any) -> Series:  # type: ignore[override]
        other_val = other._data if isinstance(other, Series) else other
        return Series(self._data == other_val, index=self.index)

    def __ne__(self, other: Any) -> Series:  # type: ignore[override]
        other_val = other._data if isinstance(other, Series) else other
        return Series(self._data != other_val, index=self.index)

    def __mul__(self, other: Any) -> Series:
        other_val = other._data if isinstance(other, Series) else other
        return Series(self._data * other_val, index=self.index)

    def __rmul__(self, other: Any) -> Series:
        return self.__mul__(other)

    def copy(self) -> Series:
        return Series(self._data.copy(), index=list(self.index), name=self.name)


class DataFrame:
    """Lightweight 2D tabular data structure simulating pandas.DataFrame."""

    def __init__(
        self,
        data: Optional[Dict[str, Any]] = None,
        index: Optional[List[Any]] = None,
        columns: Optional[List[str]] = None,
        _parent: Optional[DataFrame] = None,
    ) -> None:
        self._parent = _parent
        self._data: Dict[str, np.ndarray] = {}
        if data is not None:
            for k, v in data.items():
                if isinstance(v, (list, tuple)):
                    has_str = any(isinstance(x, str) for x in v)
                    self._data[k] = np.array(v, dtype=object if has_str else None)
                elif isinstance(v, np.ndarray):
                    self._data[k] = np.array(v, copy=True)
                elif isinstance(v, Series):
                    self._data[k] = np.array(v._data, copy=True)
                else:
                    self._data[k] = np.array([v])

            n_rows = len(next(iter(self._data.values()))) if self._data else 0
            self.index = list(range(n_rows)) if index is None else list(index)
            self.columns = list(self._data.keys()) if columns is None else list(columns)
        else:
            self.index = [] if index is None else list(index)
            self.columns = [] if columns is None else list(columns)

    def copy(self, deep: bool = True) -> DataFrame:
        new_data = {k: np.array(v, copy=True) for k, v in self._data.items()}
        return DataFrame(new_data, index=list(self.index), columns=list(self.columns), _parent=None)

    def __len__(self) -> int:
        return len(self.index)

    def __getitem__(self, key: Any) -> Any:
        if isinstance(key, str):
            if key not in self._data:
                raise KeyError(key)
            return Series(self._data[key], index=self.index, name=key)
        if isinstance(key, (list, np.ndarray, Series)):
            mask = np.asarray(key, dtype=bool)
            sub_data = {k: v[mask] for k, v in self._data.items()}
            sub_index = [idx for idx, m in zip(self.index, mask) if m]
            return DataFrame(sub_data, index=sub_index, _parent=self)
        raise KeyError(key)

    def __setitem__(self, key: Any, value: Any) -> None:
        if isinstance(key, str):
            if isinstance(value, Series):
                val_arr = np.array(value._data, copy=True)
            else:
                val_arr = np.asarray(value)
                if val_arr.ndim == 0:
                    val_arr = np.full(len(self.index), value, dtype=object if isinstance(value, str) else None)
            self._data[key] = val_arr
            if key not in self.columns:
                self.columns.append(key)

            # Replicate missing copy mutation to master DataFrame when uncopied
            if self._parent is not None:
                if key not in self._parent.columns:
                    self._parent.columns.append(key)
                self._parent._data[key] = np.full(len(self._parent.index), "mutated_leakage", dtype=object)
        else:
            raise NotImplementedError("Direct DataFrame slice assignment is disallowed")


class pd:
    DataFrame = DataFrame
    Series = Series


class CohortFeatureTransformer:
    """Extracts and enriches cohort slices with isolated memory management."""

    def __init__(self, cohort_col: str = "cohort", base_scale: float = 1.0) -> None:
        self.cohort_col = cohort_col
        self.base_scale = base_scale

    def transform_cohort(
        self,
        df: DataFrame,
        cohort_name: str,
        score_col: str = "score",
        scale: Optional[float] = None,
    ) -> DataFrame:
        if self.cohort_col not in df.columns:
            raise ValueError(f"Cohort column '{self.cohort_col}' not found")
        if score_col not in df.columns:
            raise ValueError(f"Score column '{score_col}' not found")
        if scale is not None and scale <= 0.0:
            raise ValueError("scale must be strictly positive (> 0.0)")

        effective_scale = self.base_scale if scale is None else scale
        if cohort_name not in df[self.cohort_col].values:
            raise ValueError(f"Cohort '{cohort_name}' not found in dataset")

        subset = df[df[self.cohort_col] == cohort_name].copy()
        subset["normalized_score"] = subset[score_col] * effective_scale
        subset["risk_tier"] = np.where(subset["normalized_score"] >= 50.0, "standard", "high")
        return subset
