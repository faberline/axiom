import pytest

from candidate import TagTally


def test_tags_are_normalized_and_blanks_ignored():
    tally = TagTally()
    tally.add(["Python", " python ", "  ", "rust"])
    assert tally.count("PYTHON") == 2
    assert len(tally) == 2


def test_top_breaks_ties_alphabetically():
    tally = TagTally()
    tally.add(["zeta", "alpha", "mid", "mid", "zeta", "alpha", "beta"])
    assert tally.top(3) == [("alpha", 2), ("mid", 2), ("zeta", 2)]


def test_retract_forgets_exhausted_tags():
    tally = TagTally()
    tally.add(["a", "a", "b"])
    tally.retract(["b", "b", "a"])
    assert tally.top(5) == [("a", 1)]
    assert len(tally) == 1


def test_top_zero_is_empty_and_negative_is_rejected():
    tally = TagTally()
    tally.add(["a"])
    assert tally.top(0) == []
    with pytest.raises(ValueError, match="n must not be negative"):
        tally.top(-1)
