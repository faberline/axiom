import pytest

from candidate import UnsupportedParameterError, tool_schema


def search(query: str, limit: int = 10, exact: bool = False, boost: float = 1.0):
    """Search the catalog.

    Returns matching product ids.
    """
    return [query, limit, exact, boost]


def maybe(tag: str, note: str | None = None):
    return tag, note


def test_function_metadata_comes_from_name_and_docstring():
    fn = tool_schema(search)
    assert fn["type"] == "function"
    assert fn["function"]["name"] == "search"
    assert fn["function"]["description"] == (
        "Search the catalog.\n\nReturns matching product ids."
    )


def test_parameter_types_map_to_json_schema():
    params = tool_schema(search)["function"]["parameters"]
    assert params["type"] == "object"
    assert params["properties"] == {
        "query": {"type": "string"},
        "limit": {"type": "integer"},
        "exact": {"type": "boolean"},
        "boost": {"type": "number"},
    }
    assert params["additionalProperties"] is False


def test_only_parameters_without_defaults_are_required():
    params = tool_schema(search)["function"]["parameters"]
    assert params["required"] == ["query"]


def test_variadic_parameters_are_rejected():
    def tagged(*tags: str):
        return tags

    with pytest.raises(UnsupportedParameterError, match="tags"):
        tool_schema(tagged)


def test_unsupported_or_missing_annotations_are_rejected():
    def bare(x):
        return x

    with pytest.raises(UnsupportedParameterError, match="x"):
        tool_schema(bare)
    with pytest.raises(UnsupportedParameterError, match="note"):
        tool_schema(maybe)
    assert issubclass(UnsupportedParameterError, TypeError)
