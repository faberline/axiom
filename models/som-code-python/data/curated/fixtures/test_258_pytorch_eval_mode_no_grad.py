import contextlib

import pytest

from candidate import evaluate

GRAD = {"enabled": True}


@contextlib.contextmanager
def no_grad():
    previous = GRAD["enabled"]
    GRAD["enabled"] = False
    try:
        yield
    finally:
        GRAD["enabled"] = previous


class Model:
    def __init__(self, training=True, fail_on=None):
        self.training = training
        self.fail_on = fail_on
        self.calls = []

    def eval(self):
        self.training = False

    def train(self, mode=True):
        self.training = mode

    def __call__(self, x):
        self.calls.append((x, self.training, GRAD["enabled"]))
        if x == self.fail_on:
            raise RuntimeError("boom")
        return x * 2


def mse(pred, target):
    return (pred - target) ** 2


def test_mean_loss_over_batches():
    assert evaluate(Model(), [(1, 2), (2, 2), (3, 2)], mse, no_grad) == pytest.approx(
        (0 + 4 + 16) / 3
    )


def test_forward_runs_in_eval_mode_without_grad():
    model = Model()
    evaluate(model, [(1, 2), (2, 4)], mse, no_grad)
    assert [c[1:] for c in model.calls] == [(False, False), (False, False)]
    assert GRAD["enabled"] is True


def test_training_mode_is_restored():
    model = Model(training=True)
    evaluate(model, [(1, 2)], mse, no_grad)
    assert model.training is True


def test_eval_mode_is_preserved():
    model = Model(training=False)
    evaluate(model, [(1, 2)], mse, no_grad)
    assert model.training is False


def test_mode_is_restored_when_forward_raises():
    model = Model(training=True, fail_on=2)
    with pytest.raises(RuntimeError, match="boom"):
        evaluate(model, [(1, 2), (2, 2)], mse, no_grad)
    assert model.training is True
    assert GRAD["enabled"] is True


def test_empty_batches_raise():
    model = Model()
    with pytest.raises(ValueError, match="no batches"):
        evaluate(model, [], mse, no_grad)
    assert model.training is True
