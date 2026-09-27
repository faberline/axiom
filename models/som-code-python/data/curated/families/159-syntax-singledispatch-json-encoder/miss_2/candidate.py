"""Encode Python values into JSON-ready data with functools.singledispatch."""

import math
from collections.abc import Mapping, Sequence
from datetime import date, datetime
from decimal import Decimal
from functools import singledispatch


@singledispatch
def encode(value: object) -> object:
    """Convert value to JSON-ready data; unregistered types are refused."""
    raise TypeError(f"cannot encode {type(value).__name__}")


@encode.register(type(None))
@encode.register(bool)
@encode.register(int)
@encode.register(str)
def _encode_plain(value: object) -> object:
    return value


@encode.register(float)
def _encode_float(value: float) -> object:
    if not math.isfinite(value):
        raise ValueError(f"cannot encode non-finite float {value}")
    return value


@encode.register(Decimal)
def _encode_decimal(value: Decimal) -> object:
    return str(value)


@encode.register(date)
def _encode_date(value: date) -> object:
    return value.isoformat()


@encode.register(datetime)
def _encode_datetime(value: datetime) -> object:
    if value.tzinfo is None:
        raise ValueError("cannot encode a naive datetime")
    return value.isoformat()


@encode.register(list)
def _encode_sequence(value: Sequence[object]) -> object:
    return [encode(item) for item in value]


@encode.register(dict)
def _encode_mapping(value: Mapping[object, object]) -> object:
    encoded: dict[str, object] = {}
    for key, item in value.items():
        if not isinstance(key, str):
            raise TypeError(f"keys must be strings, got {type(key).__name__}")
        encoded[key] = encode(item)
    return encoded
