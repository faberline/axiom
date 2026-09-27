import warnings

import pytest

from candidate import deprecated, fetch_all, fetch_rows

MESSAGE = "fetch_all is deprecated; use fetch_rows instead"


def test_old_name_warns_and_still_works():
    with pytest.warns(DeprecationWarning, match=MESSAGE):
        assert fetch_all(2) == [1, 2]


def test_warning_points_at_the_caller():
    with pytest.warns(DeprecationWarning) as record:
        fetch_all()
    assert record[0].filename == __file__


def test_every_call_warns():
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        fetch_all()
        fetch_all()
    assert [w.category for w in caught] == [DeprecationWarning, DeprecationWarning]


def test_wrapper_keeps_metadata():
    assert fetch_all.__name__ == "fetch_all"
    assert fetch_all.__doc__ == "Old name for fetch_rows."


def test_new_name_does_not_warn():
    with warnings.catch_warnings():
        warnings.simplefilter("error")
        assert fetch_rows() == [1, 2, 3]


def test_errors_propagate_through_the_wrapper():
    with pytest.warns(DeprecationWarning), pytest.raises(ValueError, match="negative"):
        fetch_all(-1)


def test_replacement_is_required():
    with pytest.raises(ValueError, match="replacement must be named"):
        deprecated("")
