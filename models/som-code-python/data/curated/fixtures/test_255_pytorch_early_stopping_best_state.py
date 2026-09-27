import math

import pytest

from candidate import EarlyStopping


class Model:
    def __init__(self):
        self.weights = {"w": [0.0]}

    def state_dict(self):
        return self.weights


def test_improvement_resets_the_counter_and_snapshots_state():
    model = Model()
    stopper = EarlyStopping(patience=2)
    assert stopper.step(1.0, model) is False
    assert stopper.best_state == {"w": [0.0]}
    model.weights["w"][0] = 9.0
    assert stopper.step(1.5, model) is False
    assert stopper.bad_epochs == 1
    assert stopper.step(0.5, model) is False
    assert stopper.bad_epochs == 0
    assert stopper.best_loss == 0.5


def test_best_state_is_a_deep_copy():
    model = Model()
    stopper = EarlyStopping()
    stopper.step(1.0, model)
    model.weights["w"][0] = 42.0
    assert stopper.best_state == {"w": [0.0]}


def test_stops_after_patience_bad_epochs():
    model = Model()
    stopper = EarlyStopping(patience=3)
    stopper.step(1.0, model)
    assert [stopper.step(1.0, model) for _ in range(3)] == [False, False, True]


def test_min_delta_requires_a_real_improvement():
    model = Model()
    stopper = EarlyStopping(patience=1, min_delta=0.1)
    stopper.step(1.0, model)
    assert stopper.step(0.95, model) is True
    assert stopper.best_loss == 1.0


def test_first_epoch_always_improves():
    stopper = EarlyStopping()
    assert stopper.best_loss == math.inf
    assert stopper.best_state is None
    assert stopper.step(1e9, Model()) is False
    assert stopper.best_loss == 1e9


def test_nan_loss_is_rejected():
    with pytest.raises(ValueError, match="NaN"):
        EarlyStopping().step(float("nan"), Model())


def test_arguments_are_validated():
    with pytest.raises(ValueError, match="patience"):
        EarlyStopping(patience=0)
    with pytest.raises(ValueError, match="min_delta"):
        EarlyStopping(min_delta=-0.1)
    EarlyStopping(patience=1, min_delta=0.0)
