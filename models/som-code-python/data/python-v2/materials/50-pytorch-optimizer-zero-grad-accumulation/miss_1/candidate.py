"""Gradient accumulation manager orchestrating scaled backpropagation and zero_grad cycles."""

from __future__ import annotations

from typing import Any


class GradientAccumulationManager:
    """Manages gradient accumulation steps, loss scaling, optimizer stepping, and grad clearing."""

    def __init__(
        self,
        optimizer: Any,
        accumulation_steps: int = 1,
        max_grad_norm: float | None = None,
    ) -> None:
        if not isinstance(accumulation_steps, int) or accumulation_steps < 1:
            raise ValueError(
                f"accumulation_steps must be an integer >= 1, got {accumulation_steps}"
            )
        if max_grad_norm is not None and max_grad_norm <= 0.0:
            raise ValueError(f"max_grad_norm must be positive, got {max_grad_norm}")

        self.optimizer = optimizer
        self.accumulation_steps = accumulation_steps
        self.max_grad_norm = max_grad_norm
        self.pending_steps: int = 0

    def backward_and_step(self, loss: Any, step_idx: int) -> bool:
        """Scale loss, execute backward, and trigger optimizer step on accumulation cadence."""
        if step_idx < 0:
            raise ValueError(f"step_idx must be non-negative, got {step_idx}")

        scaled_loss = loss / self.accumulation_steps
        scaled_loss.backward()
        self.pending_steps += 1

        is_step = ((step_idx + 1) % self.accumulation_steps == 0)
        if is_step:
            if self.max_grad_norm is not None and hasattr(self.optimizer, "clip_grad_norm_"):
                self.optimizer.clip_grad_norm_(self.max_grad_norm)
            self.optimizer.step()
            self.optimizer.zero_grad()
            self.pending_steps = 0
            return True

        return False

    def flush(self) -> bool:
        """Step optimizer and zero gradients for any remaining accumulated batches."""
        if self.pending_steps > 0:
            if self.max_grad_norm is not None and hasattr(self.optimizer, "clip_grad_norm_"):
                self.optimizer.clip_grad_norm_(self.max_grad_norm)
            self.optimizer.step()
            self.optimizer.zero_grad()
            self.pending_steps = 0
            return True
        return False

    def reset(self) -> None:
        """Reset pending accumulation step counter."""
        self.pending_steps = 0
