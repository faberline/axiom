import json

import pytest
from hypothesis import find, given, settings
from hypothesis import strategies as st

from candidate import canonical_dumps, depth, json_values

FIXED = settings(max_examples=200, derandomize=True, database=None, deadline=None)


@FIXED
@given(json_values())
def test_canonical_dumps_round_trips(value):
    assert json.loads(canonical_dumps(value)) == value


@FIXED
@given(st.dictionaries(st.text(max_size=4), st.integers(), max_size=6))
def test_key_order_does_not_change_the_output(mapping):
    reordered = dict(reversed(list(mapping.items())))
    assert canonical_dumps(mapping) == canonical_dumps(reordered)


def test_output_is_compact_sorted_and_raw_utf8():
    assert canonical_dumps({"b": 1, "a": [1, 2]}) == '{"a":[1,2],"b":1}'
    assert canonical_dumps("é") == '"é"'


def test_nan_and_infinity_are_rejected():
    for bad in (float("nan"), [float("inf")]):
        with pytest.raises(ValueError):
            canonical_dumps(bad)


def test_depth_counts_container_nesting():
    assert depth(1) == 0
    assert depth([]) == 1
    assert depth({}) == 1
    assert depth({"a": [{}]}) == 3
    assert depth([[], [[1]]]) == 3


def test_strategy_generates_nested_documents():
    nested = find(json_values(), lambda v: depth(v) >= 3, settings=FIXED)
    assert depth(nested) >= 3
