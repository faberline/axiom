import pytest

from candidate import RecursiveCharacterTextSplitter


def test_short_text_is_one_chunk_and_blank_is_none():
    s = RecursiveCharacterTextSplitter(chunk_size=50, chunk_overlap=0)
    assert s.split_text("hello world") == ["hello world"]
    assert s.split_text("   ") == []
    assert s.split_text("") == []


def test_paragraphs_are_preferred_over_words():
    text = "alpha beta gamma\n\ndelta epsilon zeta"
    s = RecursiveCharacterTextSplitter(chunk_size=20, chunk_overlap=0)
    assert s.split_text(text) == ["alpha beta gamma", "delta epsilon zeta"]


def test_words_merge_up_to_chunk_size():
    s = RecursiveCharacterTextSplitter(chunk_size=11, chunk_overlap=0)
    assert s.split_text("aa bb cc dd ee ff") == ["aa bb cc dd", "ee ff"]


def test_overlap_repeats_trailing_words():
    s = RecursiveCharacterTextSplitter(chunk_size=11, chunk_overlap=5)
    chunks = s.split_text("aa bb cc dd ee ff gg")
    assert chunks == ["aa bb cc dd", "cc dd ee ff", "ee ff gg"]
    assert all(len(c) <= 11 for c in chunks)


def test_long_word_falls_back_to_characters():
    s = RecursiveCharacterTextSplitter(chunk_size=4, chunk_overlap=0)
    assert s.split_text("abcdefghij") == ["abcd", "efgh", "ij"]


def test_every_chunk_respects_the_limit():
    text = ("lorem ipsum dolor sit amet consectetur " * 30).strip()
    s = RecursiveCharacterTextSplitter(chunk_size=40, chunk_overlap=10)
    chunks = s.split_text(text)
    assert len(chunks) > 5
    assert all(0 < len(c) <= 40 for c in chunks)
    assert chunks[0].startswith("lorem")
    assert chunks[-1].endswith("consectetur")


def test_invalid_configuration():
    for size, overlap in ((0, 0), (-1, 0), (10, 10), (10, 11), (10, -1)):
        with pytest.raises(ValueError):
            RecursiveCharacterTextSplitter(chunk_size=size, chunk_overlap=overlap)
    RecursiveCharacterTextSplitter(chunk_size=10, chunk_overlap=9)
