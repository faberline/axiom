"""Accumulate epoch losses as floats so no autograd graph outlives its batch."""

from __future__ import annotations

from typing import Any


class EpochLossAccumulator:
    """Accumulates batch losses and computes sample-weighted epoch averages.

    Safely detaches loss tensors using .item() to ensure computation graph
    nodes (such as autograd DAG references) are not retained in memory.
    """

    def __init__(self, record_history: bool = True) -> None:
        self.record_history = record_history
        self.reset()

    def reset(self) -> None:
        """Reset internal accumulator state for a new epoch."""
        self.total_weighted_loss: float = 0.0
        self.total_samples: int = 0
        self._history: list[float] = []

    def record_batch(self, loss: Any, batch_size: int = 1) -> float:
        """Record one batch loss, validating inputs and detaching tensor references."""
        if not isinstance(batch_size, int) or batch_size <= 0:
            raise ValueError(f"batch_size must be a positive integer, got {batch_size}")

        val = float(loss.item()) if hasattr(loss, "item") else float(loss)

        if val < 0.0:
            raise ValueError(f"Loss value must be non-negative, got {val}")

        self.total_weighted_loss += val
        self.total_samples += batch_size

        if self.record_history:
            self._history.append(val)

        return val

    def compute_epoch_average(self) -> float:
        """Return the sample-weighted average loss for the recorded epoch."""
        if self.total_samples == 0:
            raise ValueError("No batches recorded to compute epoch average")
        return self.total_weighted_loss / self.total_samples

    def get_history(self) -> list[float]:
        """Return a copy of the recorded scalar loss history."""
        return list(self._history)
