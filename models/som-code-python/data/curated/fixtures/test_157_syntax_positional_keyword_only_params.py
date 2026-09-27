import pytest

from candidate import merge_options, retry_delays


def test_default_backoff():
    assert retry_delays(4) == [0.5, 1.0, 2.0, 4.0]


def test_delays_are_capped():
    assert retry_delays(4, base=10.0, factor=3.0, cap=40.0) == [10.0, 30.0, 40.0, 40.0]


def test_tuning_knobs_are_keyword_only():
    with pytest.raises(TypeError):
        retry_delays(3, 1.0)  # type: ignore[misc]


def test_attempts_is_positional_only():
    with pytest.raises(TypeError):
        retry_delays(attempts=3)  # type: ignore[call-arg]


@pytest.mark.parametrize(
    ("kwargs", "message"),
    [
        ({"attempts": 0}, "attempts must be at least 1"),
        ({"attempts": 2, "base": 0.0}, "base must be positive"),
        ({"attempts": 2, "factor": 0.5}, "factor must be at least 1"),
    ],
)
def test_invalid_arguments(kwargs, message):
    attempts = kwargs.pop("attempts")
    with pytest.raises(ValueError, match=message):
        retry_delays(attempts, **kwargs)


def test_override_named_defaults_is_allowed():
    base = {"defaults": False, "timeout": 5}
    assert merge_options(base, defaults=True) == {"defaults": True, "timeout": 5}
    assert base == {"defaults": False, "timeout": 5}


def test_unknown_options_are_rejected():
    with pytest.raises(ValueError, match="unknown option: color, size"):
        merge_options({"timeout": 5}, size=1, color="red")
