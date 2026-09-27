import itertools

import pytest

from candidate import chunked, sliding


def test_chunks_keep_the_trailing_partial_by_default():
    assert list(chunked(range(7), 3)) == [(0, 1, 2), (3, 4, 5), (6,)]


def test_drop_partial_discards_the_short_tail():
    assert list(chunked(range(7), 3, drop_partial=True)) == [(0, 1, 2), (3, 4, 5)]
    assert list(chunked(range(6), 3, drop_partial=True)) == [(0, 1, 2), (3, 4, 5)]


def test_chunking_is_lazy_over_infinite_input():
    assert next(chunked(itertools.count(), 2)) == (0, 1)


def test_zero_size_is_rejected():
    with pytest.raises(ValueError, match="size must be at least 1"):
        list(chunked([1, 2], 0))


def test_sliding_windows_are_full_width_only():
    assert list(sliding("abcd", 3)) == [("a", "b", "c"), ("b", "c", "d")]
    assert list(sliding("ab", 3)) == []


def test_sliding_rejects_zero_width():
    with pytest.raises(ValueError, match="width must be at least 1"):
        list(sliding("abc", 0))
