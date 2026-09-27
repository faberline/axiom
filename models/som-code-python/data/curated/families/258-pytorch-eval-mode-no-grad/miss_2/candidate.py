"""Evaluate a torch-style model in eval mode under no_grad, then restore it."""

from __future__ import annotations

from collections.abc import Callable, Iterable
from contextlib import AbstractContextManager
from typing import Any


def evaluate(
    model: Any,
    batches: Iterable[tuple[Any, Any]],
    loss_fn: Callable[[Any, Any], Any],
    no_grad: Callable[[], AbstractContextManager[Any]],
) -> float:
    """Return the mean batch loss without tracking gradients."""
    was_training = model.training
    model.eval()
    total = 0.0
    count = 0
    try:
        with no_grad():
            for inputs, targets in batches:
                total += float(loss_fn(model(inputs), targets))
                count += 1
    finally:
        model.train()
    if count == 0:
        raise ValueError("no batches to evaluate")
    return total / count
