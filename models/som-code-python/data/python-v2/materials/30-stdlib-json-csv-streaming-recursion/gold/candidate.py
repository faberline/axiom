import csv
import io
import json
from typing import Any, Dict, Iterator, List


class StreamSafeDataProcessor:
    def safe_parse_json(self, json_str: str, max_depth: int = 20) -> Any:
        depth = 0
        for char in json_str:
            if char in "{[":
                depth += 1
                if depth > max_depth:
                    raise ValueError(f"JSON nesting depth exceeds maximum allowed depth of {max_depth}")
            elif char in "}]":
                depth = max(0, depth - 1)
        return json.loads(json_str)

    def stream_csv_records(self, file_obj: io.TextIOBase, batch_size: int = 100) -> Iterator[List[Dict[str, str]]]:
        if batch_size <= 0:
            raise ValueError("batch_size must be positive")
        reader = csv.DictReader(file_obj)
        batch: List[Dict[str, str]] = []
        for row in reader:
            batch.append(dict(row))
            if len(batch) >= batch_size:
                yield batch
                batch = []
        if batch:
            yield batch

    def compute_column_summary(self, file_obj: io.TextIOBase, numeric_column: str) -> Dict[str, float]:
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
                    if val < col_min:
                        col_min = val
                    if val > col_max:
                        col_max = val
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
