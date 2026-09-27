import pytest

from candidate import Tolerate


def test_listed_error_is_suppressed_and_recorded():
    with Tolerate(KeyError) as tolerate:
        raise KeyError("missing")
    assert [type(e) for e in tolerate.errors] == [KeyError]


def test_subclasses_are_suppressed_too():
    with Tolerate(LookupError) as tolerate:
        raise IndexError("out of range")
    assert isinstance(tolerate.errors[0], IndexError)


def test_other_errors_propagate():
    tolerate = Tolerate(KeyError)
    with pytest.raises(ZeroDivisionError), tolerate:
        _ = 1 / 0
    assert tolerate.errors == []


def test_budget_is_exact_across_blocks():
    tolerate = Tolerate(KeyError, budget=2)
    for _ in range(2):
        with tolerate:
            raise KeyError("again")
    with pytest.raises(KeyError), tolerate:
        raise KeyError("one too many")
    assert len(tolerate.errors) == 2


def test_clean_block_records_nothing():
    with Tolerate(KeyError) as tolerate:
        value = 42
    assert value == 42
    assert tolerate.errors == []


def test_constructor_validation():
    with pytest.raises(ValueError, match="at least one exception type"):
        Tolerate()
    with pytest.raises(ValueError, match="budget must be at least 1"):
        Tolerate(KeyError, budget=0)
    assert Tolerate(KeyError, budget=1).budget == 1
