"""Pin call shapes with positional-only and keyword-only parameters."""

from collections.abc import Mapping


def retry_delays(
    attempts: int, /, *, base: float = 0.5, factor: float = 2.0, cap: float = 30.0
) -> list[float]:
    """Exponential backoff delays; tuning knobs can only be passed by keyword."""
    if attempts < 1:
        raise ValueError("attempts must be at least 1")
    if base <= 0:
        raise ValueError("base must be positive")
    if factor < 1:
        raise ValueError("factor must be at least 1")
    return [min(cap, base * factor**i) for i in range(attempts)]


def merge_options(
    defaults: Mapping[str, object], **overrides: object
) -> dict[str, object]:
    """Return defaults updated by overrides, even an override named defaults."""
    unknown = sorted(set(overrides) - set(defaults))
    if unknown:
        raise ValueError(f"unknown option: {', '.join(unknown)}")
    merged = dict(defaults)
    merged.update(overrides)
    return merged
