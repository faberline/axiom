"""Family 43: pandas apply vectorization pitfall.

Vectorized scoring with np.where and column arithmetic, preserving index.
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

    def __add__(self, other: Any) -> Series:
        other_val = other._data if isinstance(other, Series) else other
        return Series(self._data + other_val, index=self.index)

    def __radd__(self, other: Any) -> Series:
        return self.__add__(other)

    def clip(self, upper: Optional[float] = None, lower: Optional[float] = None) -> Series:
        clipped = np.clip(self._data, a_min=lower, a_max=upper)
        return Series(clipped, index=self.index, name=self.name)

    def copy(self) -> Series:
        return Series(self._data.copy(), index=list(self.index), name=self.name)


class _ILocIndexer:
    def __init__(self, df: DataFrame) -> None:
        self._df = df

    def __getitem__(self, idx: int) -> Dict[str, Any]:
        return {col: self._df._data[col][idx] for col in self._df.columns}


class DataFrame:
    """Lightweight 2D tabular data structure simulating pandas.DataFrame."""

    def __init__(
        self,
        data: Optional[Dict[str, Any]] = None,
        index: Optional[List[Any]] = None,
        columns: Optional[List[str]] = None,
    ) -> None:
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

    @property
    def empty(self) -> bool:
        return len(self.index) == 0

    @property
    def iloc(self) -> _ILocIndexer:
        return _ILocIndexer(self)

    def copy(self, deep: bool = True) -> DataFrame:
        new_data = {k: np.array(v, copy=True) for k, v in self._data.items()}
        return DataFrame(new_data, index=list(self.index), columns=list(self.columns))

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
            return DataFrame(sub_data, index=sub_index)
        raise KeyError(key)

    def __setitem__(self, key: Any, value: Any) -> None:
        if isinstance(key, str):
            val_arr = np.asarray(value)
            if val_arr.ndim == 0:
                val_arr = np.full(len(self.index), value, dtype=object if isinstance(value, str) else None)
            self._data[key] = val_arr
            if key not in self.columns:
                self.columns.append(key)
        else:
            raise NotImplementedError("Direct DataFrame slice assignment is disallowed")


class pd:
    DataFrame = DataFrame
    Series = Series


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
        scores = Series(base, index=df.index).clip(upper=self.max_cap)
        return scores
