"""Oracle test suite for Family 50: PyTorch Optimizer Zero Grad & Accumulation."""

from __future__ import annotations

import pytest
from candidate import GradientAccumulationManager


class MockLoss:
    """Mock loss tensor that tracks division and backward calls."""

    last_backward_value: float | None = None

    def __init__(self, value: float) -> None:
        self.value = float(value)

    def __truediv__(self, other: float) -> 'MockLoss':
        return MockLoss(self.value / other)

    def __mul__(self, other: float) -> 'MockLoss':
        return MockLoss(self.value * other)

    def backward(self) -> None:
        MockLoss.last_backward_value = self.value


class MockOptimizer:
    """Mock PyTorch optimizer recording step() and zero_grad() invocations."""

    def __init__(self) -> None:
        self.step_count = 0
        self.zero_grad_count = 0

    def step(self) -> None:
        self.step_count += 1

    def zero_grad(self) -> None:
        self.zero_grad_count += 1


def test_gold_accumulation_cadence_and_zero_grad():
    opt = MockOptimizer()
    manager = GradientAccumulationManager(opt, accumulation_steps=4)

    for i in range(3):
        stepped = manager.backward_and_step(MockLoss(1.0), step_idx=i)
        assert not stepped, f"Premature step at step_idx {i}"
        assert opt.step_count == 0
        assert opt.zero_grad_count == 0

    stepped = manager.backward_and_step(MockLoss(1.0), step_idx=3)
    assert stepped is True, "Expected optimizer step at step_idx 3"
    assert opt.step_count == 1
    assert opt.zero_grad_count == 1


def test_default_accumulation_steps_is_valid_positive():
    opt = MockOptimizer()
    manager = GradientAccumulationManager(opt)
    # Default accumulation steps should be standard batch accumulation window 4
    assert manager.accumulation_steps == 4, (
        f"Expected default accumulation_steps=4, got {manager.accumulation_steps}"
    )


def test_invalid_accumulation_steps_rejected():
    opt = MockOptimizer()

    with pytest.raises(ValueError, match="accumulation_steps"):
        GradientAccumulationManager(opt, accumulation_steps=0)

    with pytest.raises(ValueError, match="accumulation_steps"):
        GradientAccumulationManager(opt, accumulation_steps=-2)


def test_boundary_first_step_does_not_trigger_premature_step():
    opt = MockOptimizer()
    manager = GradientAccumulationManager(opt, accumulation_steps=4)

    # First step (step_idx = 0) must NOT trigger an optimizer step
    stepped = manager.backward_and_step(MockLoss(1.0), step_idx=0)
    assert not stepped, "Step 0 erroneously triggered optimizer step"
    assert opt.step_count == 0
    assert opt.zero_grad_count == 0


def test_loss_scaled_by_accumulation_factor():
    opt = MockOptimizer()
    manager = GradientAccumulationManager(opt, accumulation_steps=4)

    MockLoss.last_backward_value = None
    manager.backward_and_step(MockLoss(8.0), step_idx=0)

    # Loss of 8.0 divided by 4 accumulation steps must equal 2.0
    assert MockLoss.last_backward_value == 2.0, (
        f"Expected scaled backward loss of 2.0, got {MockLoss.last_backward_value}"
    )


def test_zero_grad_called_after_optimizer_step():
    opt = MockOptimizer()
    manager = GradientAccumulationManager(opt, accumulation_steps=4)

    for i in range(4):
        manager.backward_and_step(MockLoss(1.0), step_idx=i)

    assert opt.step_count == 1
    assert opt.zero_grad_count == 1, "optimizer.zero_grad() was not called when optimizer stepped!"


def test_flush_triggers_pending_step_and_zero_grad():
    opt = MockOptimizer()
    manager = GradientAccumulationManager(opt, accumulation_steps=4)

    # Run 2 steps out of 4
    manager.backward_and_step(MockLoss(1.0), step_idx=0)
    manager.backward_and_step(MockLoss(1.0), step_idx=1)
    assert opt.step_count == 0

    # Flush remaining accumulated batches
    flushed = manager.flush()
    assert flushed is True
    assert opt.step_count == 1
    assert opt.zero_grad_count == 1

    # Second flush has nothing pending
    assert manager.flush() is False
