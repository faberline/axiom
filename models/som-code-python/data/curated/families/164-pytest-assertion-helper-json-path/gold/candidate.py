"""A pytest assertion helper that reports the JSON path of the first mismatch."""


def assert_subset(actual: object, expected: object, path: str = "$") -> None:
    """Assert expected is contained in actual, recursing into dicts and lists.

    Dicts may carry extra keys; lists must match item for item; scalars must
    be equal and of the same type, so True never stands in for 1.
    """
    __tracebackhide__ = True  # pylint: disable=unused-variable
    if isinstance(expected, dict):
        if not isinstance(actual, dict):
            raise AssertionError(
                f"{path}: expected an object, got {type(actual).__name__}"
            )
        for key, value in expected.items():
            if key not in actual:
                raise AssertionError(f"{path}: missing key {key!r}")
            assert_subset(actual[key], value, f"{path}.{key}")
    elif isinstance(expected, list):
        if not isinstance(actual, list) or len(actual) != len(expected):
            got = len(actual) if isinstance(actual, list) else type(actual).__name__
            raise AssertionError(f"{path}: expected {len(expected)} items, got {got}")
        for index, (item, want) in enumerate(zip(actual, expected, strict=True)):
            assert_subset(item, want, f"{path}[{index}]")
    elif type(actual) is not type(expected) or actual != expected:
        raise AssertionError(f"{path}: {actual!r} != {expected!r}")
