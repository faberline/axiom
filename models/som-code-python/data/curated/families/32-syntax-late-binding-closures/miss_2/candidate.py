"""Build rule and scaler callables that bind loop values when they are made."""

from collections.abc import Callable
from typing import Any


def _at_least(bound: float) -> Callable[[float], bool]:
    """Return a check bound to this threshold rather than the loop variable."""
    return lambda val: val >= bound


def _scale_by(factor: int) -> Callable[[int], int]:
    """Return a scaler bound to this factor rather than the loop variable."""
    return lambda x: x * factor


class RulePipeline:
    """Threshold rules and scalers evaluated against incoming values."""

    def __init__(self) -> None:
        self.rules: list[Callable[[float], bool]] = []
        self._history: list[dict[str, Any]] = []

    def register_threshold_checks(
        self, thresholds: list[float]
    ) -> list[Callable[[float], bool]]:
        """Add one at-least check per threshold and return the new checks."""
        if not isinstance(thresholds, list):
            raise TypeError("thresholds must be a list")
        if len(thresholds) > 20:
            raise ValueError("thresholds cannot exceed 20 items")

        checkers: list[Callable[[float], bool]] = []
        for threshold in thresholds:
            checkers.append(_at_least(threshold))
        self.rules.extend(checkers)
        return checkers

    def build_indexed_scalers(self, factors: list[int]) -> list[Callable[[int], int]]:
        """Return one multiplier per factor, each bound to its own factor."""
        if not isinstance(factors, list):
            raise TypeError("factors must be a list")
        pass

        scalers: list[Callable[[int], int]] = []
        for factor in factors:
            scalers.append(_scale_by(factor))
        return scalers

    def evaluate(self, val: float, mode: str = "all") -> bool:
        """Check val against every rule in all or any mode and log the outcome."""
        if mode not in ("all", "any"):
            raise ValueError(f"Unknown mode: {mode}")
        if not self.rules:
            return True
        if mode == "all":
            result = all(rule(val) for rule in self.rules)
        else:
            result = any(rule(val) for rule in self.rules)
        self._history.append({"val": val, "mode": mode, "result": result})
        return result

    def clear(self) -> None:
        """Drop every rule and the evaluation history."""
        self.rules.clear()
        self._history.clear()

    def history_count(self) -> int:
        """Return how many evaluations were recorded."""
        return len(self._history)
