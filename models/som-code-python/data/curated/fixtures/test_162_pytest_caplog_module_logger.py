import logging

import pytest

from candidate import parse_amounts


@pytest.fixture
def logs(caplog):
    caplog.set_level(logging.INFO, logger="candidate")
    return caplog


def test_clean_input_logs_only_a_summary(logs):
    assert parse_amounts(["5", "", "0", " 7 "]) == [5, 0, 7]
    assert [(r.levelname, r.getMessage()) for r in logs.records] == [
        ("INFO", "parsed 3 of 3 lines"),
    ]


def test_bad_lines_are_warned_with_their_line_number(logs):
    assert parse_amounts(["10", "abc", "", "-4", "2"]) == [10, 2]
    warnings = [r for r in logs.records if r.levelno == logging.WARNING]
    assert [r.getMessage() for r in warnings] == [
        "line 2: not an integer: 'abc'",
        "line 4: negative amount -4 skipped",
    ]
    assert logs.records[-1].getMessage() == "parsed 2 of 4 lines"


def test_records_come_from_the_module_logger(logs):
    parse_amounts(["x", "-1", "3"])
    assert {r.name for r in logs.records} == {"candidate"}


def test_messages_are_formatted_lazily(logs):
    parse_amounts(["x", "1"])
    record = logs.records[0]
    assert record.msg == "line %d: not an integer: %r"
    assert record.args == (1, "x")


def test_no_valid_amounts_logs_error_and_raises(logs):
    with pytest.raises(ValueError, match="no valid amounts in input"):
        parse_amounts(["-1", "nope"])
    assert logs.records[-1].levelname == "ERROR"
    assert logs.records[-1].getMessage() == "no valid amounts in 2 lines"
