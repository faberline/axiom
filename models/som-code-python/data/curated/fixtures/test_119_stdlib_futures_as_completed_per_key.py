import threading
import time

import pytest

from candidate import fetch_all


def test_each_key_gets_a_value_or_an_error():
    def fetch(key):
        if key == "b":
            raise KeyError(key)
        return key.upper()

    report = fetch_all(["a", "b", "c"], fetch)
    assert report.values == {"a": "A", "c": "C"}
    assert report.errors == {"b": "KeyError: 'b'"}


def test_duplicate_keys_are_fetched_once():
    calls = []
    lock = threading.Lock()

    def fetch(key):
        with lock:
            calls.append(key)
        return len(key)

    report = fetch_all(["aa", "aa", "b"], fetch)
    assert sorted(calls) == ["aa", "b"]
    assert report.values == {"aa": 2, "b": 1}


def _peak(**kwargs):
    state = {"now": 0, "peak": 0}
    lock = threading.Lock()

    def fetch(key):
        with lock:
            state["now"] += 1
            state["peak"] = max(state["peak"], state["now"])
        time.sleep(0.05)
        with lock:
            state["now"] -= 1
        return key

    fetch_all([str(i) for i in range(8)], fetch, **kwargs)
    return state["peak"]


def test_pool_size_bounds_concurrency():
    assert _peak(max_workers=2) == 2
    assert _peak() == 4


def test_pool_size_must_be_positive():
    with pytest.raises(ValueError, match="max_workers must be at least 1"):
        fetch_all(["a"], str, max_workers=0)
