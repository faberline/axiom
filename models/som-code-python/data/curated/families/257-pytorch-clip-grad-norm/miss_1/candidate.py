"""Global gradient norm clipping in the style of torch's clip_grad_norm_."""

from __future__ import annotations

import math
from collections.abc import Iterable
from typing import Any

EPS = 1e-6


def clip_grad_norm(
    params: Iterable[Any], max_norm: float, *, error_if_nonfinite: bool = True
) -> float:
    """Scale every gradient in place so their joint L2 norm is at most max_norm."""
    if max_norm <= 0:
        raise ValueError("max_norm must be positive")
    grads: list[list[float]] = [p.grad for p in params]
    total = math.sqrt(sum(g * g for grad in grads for g in grad))
    if not math.isfinite(total):
        if error_if_nonfinite:
            raise RuntimeError("total gradient norm is non-finite")
        return total
    coef = max_norm / (total + EPS)
    if coef < 1.0:
        for grad in grads:
            grad[:] = [g * coef for g in grad]
    return total
