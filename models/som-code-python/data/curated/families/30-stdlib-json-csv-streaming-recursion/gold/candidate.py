"""Parse JSON and stream CSV without unbounded recursion or memory."""

import csv
import io
import json
from collections.abc import Iterator
from typing import Any


class StreamSafeDataProcessor:
    """Bounded-depth JSON parsing and batched CSV streaming."""

    def safe_parse_json(self, json_str: str, max_depth: int = 20) -> Any:
        """Parse json_str, refusing nesting deeper than max_depth."""
        depth = 0
        for char in json_str:
            if char in "{[":
                depth += 1
                if depth > max_depth:
                    raise ValueError(
                        "JSON nesting depth exceeds maximum allowed depth of "
                        f"{max_depth}"
                    )
            elif char in "}]":
                depth = max(0, depth - 1)
        return json.loads(json_str)

    def stream_csv_records(
        self, file_obj: io.TextIOBase, batch_size: int = 100
    ) -> Iterator[list[dict[str, str]]]:
        """Yield CSV rows in batches of at most batch_size."""
        if batch_size <= 0:
            raise ValueError("batch_size must be positive")
        reader = csv.DictReader(file_obj)
        batch: list[dict[str, str]] = []
        for row in reader:
            batch.append(dict(row))
            if len(batch) >= batch_size:
                yield batch
                batch = []
        if batch:
            yield batch

    def compute_column_summary(
        self, file_obj: io.TextIOBase, numeric_column: str
    ) -> dict[str, float]:
        """Summarize one numeric column in a single pass, skipping bad cells."""
        reader = csv.DictReader(file_obj)
        count = 0
        total = 0.0
        col_min = float("inf")
        col_max = float("-inf")
        for row in reader:
            val_str = row.get(numeric_column)
            if val_str is not None and val_str.strip() != "":
                try:
                    val = float(val_str)
                    total += val
                    count += 1
                    col_min = min(col_min, val)
                    col_max = max(col_max, val)
                except ValueError:
                    continue
        if count == 0:
            return {"count": 0.0, "sum": 0.0, "mean": 0.0, "min": 0.0, "max": 0.0}
        return {
            "count": float(count),
            "sum": total,
            "mean": total / count,
            "min": col_min,
            "max": col_max,
        }
