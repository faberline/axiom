"""A linear-warmup, cosine-decay learning rate factor for LambdaLR."""

from __future__ import annotations

import math
from collections.abc import Callable


def warmup_cosine(
    warmup_steps: int, total_steps: int, min_ratio: float = 0.0
) -> Callable[[int], float]:
    """Return lr_lambda(step) giving the multiplier of the base learning rate."""
    if warmup_steps < 0:
        raise ValueError("warmup_steps must be non-negative")
    if total_steps <= warmup_steps:
        raise ValueError("total_steps must exceed warmup_steps")
    if not 0.0 <= min_ratio <= 1.0:
        raise ValueError("min_ratio must be within [0, 1]")
    decay_steps = total_steps - warmup_steps

    def lr_lambda(step: int) -> float:
        if step < warmup_steps:
            return step / warmup_steps
        progress = min(1.0, (step - warmup_steps) / decay_steps)
        cosine = 0.5 * (1.0 + math.cos(math.pi * progress))
        return min_ratio + (1.0 - min_ratio) * cosine

    return lr_lambda
