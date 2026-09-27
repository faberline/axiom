"""Oracle test suite for Family 49: PyTorch Tensor Graph Memory Leak Prevention."""

import inspect
import pytest
from candidate import EpochLossAccumulator


class MockTensor:
    """Mock PyTorch tensor object tracking computation graph autograd DAG references."""

    def __init__(self, value: float, has_grad_fn: bool = True) -> None:
        self.value = float(value)
        self.grad_fn = "<MseLossBackward0>" if has_grad_fn else None

    def item(self) -> float:
        return self.value

    def __repr__(self) -> str:
        return f"tensor({self.value}, grad_fn={self.grad_fn})"


def test_gold_batch_recording_and_weighted_average():
    acc = EpochLossAccumulator(record_history=True)
    t1 = MockTensor(1.5)
    t2 = MockTensor(2.5)

    v1 = acc.record_batch(t1, batch_size=4)
    v2 = acc.record_batch(t2, batch_size=6)

    assert v1 == 1.5
    assert v2 == 2.5
    # Weighted average: (1.5*4 + 2.5*6) / (4 + 6) = (6.0 + 15.0) / 10 = 2.1
    assert abs(acc.compute_epoch_average() - 2.1) < 1e-7

    history = acc.get_history()
    assert history == [1.5, 2.5]

    acc.reset()
    assert acc.get_history() == []
    assert acc.total_samples == 0


def test_default_batch_size_is_one():
    acc = EpochLossAccumulator()
    sig = inspect.signature(acc.record_batch)
    assert sig.parameters["batch_size"].default == 1, (
        f"Expected default batch_size=1, got {sig.parameters['batch_size'].default}"
    )

    t = MockTensor(3.0)
    acc.record_batch(t)  # Must use default batch_size=1 without raising error
    assert acc.total_samples == 1
    assert acc.compute_epoch_average() == 3.0


def test_invalid_batch_size_rejected():
    acc = EpochLossAccumulator()
    t = MockTensor(1.0)

    with pytest.raises(ValueError, match="positive integer"):
        acc.record_batch(t, batch_size=0)

    with pytest.raises(ValueError, match="positive integer"):
        acc.record_batch(t, batch_size=-4)


def test_boundary_zero_loss_accepted():
    acc = EpochLossAccumulator()
    t_zero = MockTensor(0.0)

    val = acc.record_batch(t_zero, batch_size=8)
    assert val == 0.0
    assert acc.compute_epoch_average() == 0.0


def test_sample_weighted_average_uneven_batches():
    acc = EpochLossAccumulator()
    # Batch 1: 10 samples with loss 2.0
    # Batch 2: 2 samples with loss 5.0
    # Weighted avg: (20 + 10) / 12 = 2.5
    acc.record_batch(MockTensor(2.0), batch_size=10)
    acc.record_batch(MockTensor(5.0), batch_size=2)

    avg = acc.compute_epoch_average()
    assert abs(avg - 2.5) < 1e-7, f"Expected weighted average 2.5, got {avg}"


def test_history_contains_only_primitive_floats_no_grad_fn():
    acc = EpochLossAccumulator(record_history=True)
    t = MockTensor(4.2, has_grad_fn=True)

    acc.record_batch(t, batch_size=2)
    history = acc.get_history()

    assert len(history) == 1
    recorded_item = history[0]

    # Must be primitive Python float, never retaining tensor or grad_fn DAG
    assert type(recorded_item) is float, f"Expected primitive float, got {type(recorded_item)}"
    assert not hasattr(recorded_item, "grad_fn"), "Memory leak detected: grad_fn retained in history!"
