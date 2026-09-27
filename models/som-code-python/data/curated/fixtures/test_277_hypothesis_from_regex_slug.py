import pytest
from hypothesis import given, settings
from hypothesis import strategies as st

from candidate import SlugError, is_slug, slugify, slugs

FIXED = settings(max_examples=200, derandomize=True, database=None, deadline=None)


@FIXED
@given(slugs())
def test_generated_slugs_are_valid_and_fixed_points(slug):
    assert is_slug(slug)
    assert slugify(slug) == slug


@FIXED
@given(st.text(max_size=80))
def test_slugify_output_is_always_a_slug(title):
    try:
        slug = slugify(title)
    except SlugError:
        return
    assert is_slug(slug)


def test_accents_and_punctuation_are_folded():
    assert slugify("Crème Brûlée!") == "creme-brulee"
    assert slugify("  --Hello,   World--  ") == "hello-world"


def test_truncation_never_leaves_a_trailing_dash():
    assert slugify("a" * 39 + " b") == "a" * 39


def test_titles_without_slug_characters_raise():
    assert issubclass(SlugError, ValueError)
    for title in ("!!!", "", "日本"):
        with pytest.raises(SlugError):
            slugify(title)


def test_is_slug_rejects_malformed_text():
    for text in ("a--b", "-a", "a-", "A", "a b", "a" * 41):
        assert not is_slug(text)
    assert is_slug("a1-b2")
