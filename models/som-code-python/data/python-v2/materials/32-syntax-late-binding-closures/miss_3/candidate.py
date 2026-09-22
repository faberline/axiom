from collections.abc import Callable
from typing import Any


class RulePipeline:
    def __init__(self) -> None:
        self.rules: list[Callable[[float], bool]] = []
        self._history: list[dict[str, Any]] = []

    def register_threshold_checks(self, thresholds: list[float]) -> list[Callable[[float], bool]]:
        if not isinstance(thresholds, list):
            raise TypeError("thresholds must be a list")
        if len(thresholds) >= 20:
            raise ValueError("thresholds cannot exceed 20 items")

        checkers: list[Callable[[float], bool]] = []
        for threshold in thresholds:
            checkers.append(lambda val, bound=threshold: val >= bound)
        self.rules.extend(checkers)
        return checkers

    def build_indexed_scalers(self, factors: list[int]) -> list[Callable[[int], int]]:
        if not isinstance(factors, list):
            raise TypeError("factors must be a list")
        if not factors:
            raise ValueError("factors cannot be empty")

        scalers: list[Callable[[int], int]] = []
        for factor in factors:
            scalers.append(lambda x, f=factor: x * f)
        return scalers

    def evaluate(self, val: float, mode: str = 'all') -> bool:
        if mode not in ('all', 'any'):
            raise ValueError(f"Unknown mode: {mode}")
        if not self.rules:
            return True
        if mode == 'all':
            result = all(rule(val) for rule in self.rules)
        else:
            result = any(rule(val) for rule in self.rules)
        self._history.append({"val": val, "mode": mode, "result": result})
        return result

    def clear(self) -> None:
        self.rules.clear()
        self._history.clear()

    def history_count(self) -> int:
        return len(self._history)
