import numpy as np
import pytest

from candidate import train_test_indices


def test_same_seed_same_split_and_different_seed_differs():
    a = train_test_indices(100, 0.2, seed=7)
    b = train_test_indices(100, 0.2, seed=7)
    c = train_test_indices(100, 0.2, seed=8)
    np.testing.assert_array_equal(a[1], b[1])
    assert a[1].tolist() != c[1].tolist()


def test_split_is_a_sorted_partition():
    train, test = train_test_indices(10, 0.3, seed=1)
    assert len(test) == 3
    assert sorted(train.tolist() + test.tolist()) == list(range(10))
    assert train.tolist() == sorted(train.tolist())
    assert test.tolist() == sorted(test.tolist())


def test_both_sides_keep_at_least_one_sample():
    train, test = train_test_indices(10, 0.01, seed=1)
    assert (len(train), len(test)) == (9, 1)
    train, test = train_test_indices(3, 0.9, seed=1)
    assert (len(train), len(test)) == (1, 2)


def test_global_random_state_is_untouched():
    np.random.seed(123)
    expected = np.random.rand()
    np.random.seed(123)
    train_test_indices(50, 0.5, seed=0)
    assert np.random.rand() == expected


def test_invalid_arguments_are_rejected():
    for fraction in (0.0, 1.0):
        with pytest.raises(ValueError, match="between 0 and 1"):
            train_test_indices(10, fraction, seed=0)
    with pytest.raises(ValueError, match="at least 2"):
        train_test_indices(1, 0.5, seed=0)
