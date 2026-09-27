import pytest

from candidate import count_words, merge_counts, word_report


def test_chord_merges_every_document():
    docs = ["The cat sat.", "the dog sat", "A cat! THE end"]
    result = word_report(docs, top=3).apply_async()
    assert result.get() == [["the", 3], ["cat", 2], ["sat", 2]]


def test_single_document_and_top_limit():
    assert word_report(["b a b"], top=1).apply_async().get() == [["b", 2]]


def test_ties_are_broken_alphabetically():
    assert merge_counts([{"pear": 1, "apple": 1}, {"fig": 1}], 3) == [
        ["apple", 1],
        ["fig", 1],
        ["pear", 1],
    ]


def test_count_words_is_case_insensitive():
    assert count_words("Don't STOP don't") == {"don't": 2, "stop": 1}


def test_invalid_arguments():
    with pytest.raises(ValueError):
        word_report([])
    with pytest.raises(ValueError):
        word_report(["x"], top=0)
