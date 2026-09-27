import pytest

from candidate import Duration


@pytest.mark.parametrize(
    ("seconds", "text"),
    [
        (3723, "1h 2m 3s"),
        (59, "59s"),
        (0, "0s"),
        (120, "2m 0s"),
        (3600, "1h 0m 0s"),
        (3605, "1h 0m 5s"),
    ],
)
def test_default_spec(seconds, text):
    assert f"{Duration(seconds)}" == text


def test_str_uses_default_spec():
    assert str(Duration(61)) == "1m 1s"


def test_clock_spec():
    assert f"{Duration(3723):clock}" == "01:02:03"
    assert f"{Duration(90061):clock}" == "25:01:01"
    assert format(Duration(5), "clock") == "00:00:05"


def test_seconds_spec():
    assert f"{Duration(3723):s}" == "3723"


def test_unknown_spec_is_rejected():
    with pytest.raises(ValueError, match="unknown format spec: 'x'"):
        f"{Duration(10):x}"


def test_negative_duration_is_rejected():
    with pytest.raises(ValueError, match="must not be negative"):
        Duration(-1)
