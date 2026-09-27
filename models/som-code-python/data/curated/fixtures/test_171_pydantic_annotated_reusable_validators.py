import pytest
from pydantic import ValidationError

from candidate import SearchQuery


def test_comma_string_is_split_normalized_and_sorted():
    query = SearchQuery(owner="Alice", tags="Rust, python ,rust")
    assert query.owner == "alice"
    assert query.tags == ["python", "rust"]


def test_lists_are_normalized_too():
    assert SearchQuery(owner="bob", exclude=["B", "a", "b"]).exclude == ["a", "b"]


def test_blank_parts_are_dropped():
    assert SearchQuery(owner="bob", tags="a,, b ,").tags == ["a", "b"]


def test_five_distinct_tags_are_allowed():
    assert len(SearchQuery(owner="bob", tags="a,b,c,d,e,A").tags) == 5


def test_six_distinct_tags_are_rejected():
    with pytest.raises(ValidationError, match="at most 5 distinct tags"):
        SearchQuery(owner="bob", tags="a,b,c,d,e,f")


@pytest.mark.parametrize("owner", ["al", "x" * 21])
def test_owner_length_is_bounded(owner):
    with pytest.raises(ValidationError):
        SearchQuery(owner=owner)


def test_minimum_length_owner_is_accepted():
    assert SearchQuery(owner="Ann").owner == "ann"


def test_tags_serialize_back_to_comma_strings():
    query = SearchQuery(owner="Carol", tags=["y", "x"])
    assert query.model_dump() == {"owner": "carol", "tags": "x,y", "exclude": ""}
