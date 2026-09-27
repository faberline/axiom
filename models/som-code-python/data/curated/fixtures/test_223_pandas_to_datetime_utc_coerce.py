import pandas as pd

from candidate import normalize_events

EVENTS = pd.DataFrame(
    {
        "id": [3, 1, 90, 2, 91, 92],
        "ts": [
            "2024-03-01T07:00:00-05:00",
            "2024-03-01T10:00:00+02:00",
            "not a date",
            "2024-03-01T09:30:00Z",
            "",
            None,
        ],
    }
)


def test_mixed_offsets_are_converted_to_utc():
    clean, _ = normalize_events(EVENTS)
    assert str(clean["ts"].dt.tz) == "UTC"
    assert clean["ts"].dt.strftime("%H:%M").tolist() == ["08:00", "09:30", "12:00"]


def test_clean_rows_are_sorted_by_instant():
    clean, _ = normalize_events(EVENTS)
    assert clean["id"].tolist() == [1, 2, 3]
    assert clean.index.tolist() == [0, 1, 2]


def test_unparseable_rows_are_quarantined():
    _, bad = normalize_events(EVENTS)
    assert bad["id"].tolist() == [90, 91, 92]
    assert bad.index.tolist() == [0, 1, 2]
    assert bad["ts"].iloc[0] == "not a date"


def test_input_is_not_modified():
    before = EVENTS.copy()
    normalize_events(EVENTS)
    pd.testing.assert_frame_equal(EVENTS, before)
