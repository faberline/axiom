import pytest

from candidate import Entry, top_k


def names(entries):
    return [entry.name for entry in entries]


def test_returns_highest_scores_in_descending_order():
    entries = [Entry("a", 3), Entry("b", 5), Entry("c", 1), Entry("d", 4)]
    assert names(top_k(entries, 2)) == ["b", "d"]


def test_ties_keep_the_earlier_entry():
    entries = [Entry("a", 1), Entry("b", 2), Entry("c", 2), Entry("d", 2)]
    assert names(top_k(entries, 2)) == ["b", "c"]


def test_k_larger_than_input_returns_everything_sorted():
    assert names(top_k([Entry("a", 1), Entry("b", 3)], 5)) == ["b", "a"]


def test_accepts_a_generator_and_zero_k():
    assert names(top_k((Entry(str(i), i) for i in range(10)), 3)) == ["9", "8", "7"]
    assert top_k([Entry("a", 1)], 0) == []


def test_negative_k_is_rejected():
    with pytest.raises(ValueError, match="k must not be negative"):
        top_k([Entry("a", 1)], -1)
