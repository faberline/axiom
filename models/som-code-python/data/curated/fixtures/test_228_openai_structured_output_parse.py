import pytest
from pydantic import BaseModel, ValidationError

from candidate import (
    RefusalError,
    SchemaMismatchError,
    StructuredOutputError,
    TruncatedOutputError,
    parse_structured,
)


class Invoice(BaseModel):
    number: str
    total: float


def response(content, finish="stop", refusal=None):
    message = {"role": "assistant", "content": content, "refusal": refusal}
    return {"choices": [{"index": 0, "message": message, "finish_reason": finish}]}


def test_valid_content_is_parsed():
    invoice = parse_structured(response('{"number": "A1", "total": 9.5}'), Invoice)
    assert invoice == Invoice(number="A1", total=9.5)


def test_refusal_raises_with_reason():
    with pytest.raises(RefusalError) as info:
        parse_structured(response(None, refusal="I can't help"), Invoice)
    assert info.value.reason == "I can't help"


def test_length_cutoff_is_truncation_not_schema_error():
    with pytest.raises(TruncatedOutputError):
        parse_structured(response('{"number": "A1", "to', finish="length"), Invoice)


def test_wrong_shape_is_schema_mismatch():
    with pytest.raises(SchemaMismatchError) as info:
        parse_structured(response('{"number": "A1", "total": "lots"}'), Invoice)
    assert isinstance(info.value.__cause__, ValidationError)


def test_missing_or_malformed_content_is_schema_mismatch():
    with pytest.raises(SchemaMismatchError):
        parse_structured(response(None), Invoice)
    with pytest.raises(SchemaMismatchError):
        parse_structured(response("{"), Invoice)


def test_errors_share_one_base_class():
    for error in (RefusalError, TruncatedOutputError, SchemaMismatchError):
        assert issubclass(error, StructuredOutputError)
