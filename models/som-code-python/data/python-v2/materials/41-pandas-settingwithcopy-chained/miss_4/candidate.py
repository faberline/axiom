"""Family 41: pandas SettingWithCopyWarning and chained indexing.

Uses safe .loc indexing, validates inputs, and respects inplace flag.
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

    def copy(self) -> Series:
        return Series(self._data.copy(), index=list(self.index), name=self.name)


class _LocIndexer:
    def __init__(self, df: DataFrame) -> None:
        self._df = df

    def __getitem__(self, key: Any) -> Any:
        if isinstance(key, tuple):
            row_idx, col_idx = key
            if isinstance(row_idx, (int, str)):
                pos = self._df.index.index(row_idx)
                return self._df._data[col_idx][pos]
            sub = self._df[row_idx]
            return sub[col_idx]
        return self._df[key]

    def __setitem__(self, key: Any, value: Any) -> None:
        if isinstance(key, tuple):
            row_mask, col_name = key
            if col_name not in self._df.columns:
                raise KeyError(col_name)
            mask = np.asarray(row_mask, dtype=bool)
            target_arr = self._df._data[col_name]
            if not isinstance(value, (int, float, np.number)) and target_arr.dtype.kind in ("i", "f"):
                target_arr = np.array(target_arr, dtype=object)
                self._df._data[col_name] = target_arr
            for i, m in enumerate(mask):
                if m:
                    target_arr[i] = value
        else:
            raise NotImplementedError("Single key loc assignment not supported")


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
    def loc(self) -> _LocIndexer:
        return _LocIndexer(self)

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


class ConditionalRecordUpdater:
    """Conditionally updates records in a DataFrame using safe .loc indexing."""

    def __init__(self, default_col: str = "status") -> None:
        self.default_col = default_col

    def update_records(
        self,
        df: DataFrame,
        filter_col: str,
        threshold: float,
        update_col: str,
        new_value: Any,
        inplace: bool = False,
    ) -> DataFrame:
        if not isinstance(df, DataFrame):
            raise TypeError("df must be an instance of DataFrame")
        if filter_col not in df.columns:
            raise ValueError(f"filter_col '{filter_col}' not found in columns")
        if update_col not in df.columns:
            raise ValueError(f"update_col '{update_col}' not found in columns")
        if threshold < 0.0:
            raise ValueError("threshold must be non-negative (>= 0.0)")

        target_df = df if inplace else df.copy()
        mask = target_df[filter_col] < threshold
        target_df.loc[mask, update_col] = new_value
        return target_df
