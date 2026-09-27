import pytest
from hypothesis import settings
from hypothesis.stateful import run_state_machine_as_test

from candidate import LRUCache, LRUCacheMachine

FIXED = settings(
    max_examples=100, stateful_step_count=30, derandomize=True, database=None
)


def test_cache_matches_the_model_under_random_operations():
    run_state_machine_as_test(LRUCacheMachine, settings=FIXED)


def test_get_refreshes_recency():
    cache = LRUCache(2)
    cache.put("a", 1)
    cache.put("b", 2)
    assert cache.get("a") == 1
    cache.put("c", 3)
    assert cache.keys() == ["a", "c"]
    assert cache.get("b") is None


def test_overwrite_refreshes_recency_and_value():
    cache = LRUCache(2)
    cache.put("a", 1)
    cache.put("b", 2)
    cache.put("a", 10)
    cache.put("c", 3)
    assert cache.keys() == ["a", "c"]
    assert cache.get("a") == 10


def test_cache_holds_exactly_capacity_items():
    cache = LRUCache(3)
    for index, key in enumerate("abcd"):
        cache.put(key, index)
    assert cache.keys() == ["b", "c", "d"]


def test_capacity_must_be_positive():
    for capacity in (0, -1):
        with pytest.raises(ValueError):
            LRUCache(capacity)
