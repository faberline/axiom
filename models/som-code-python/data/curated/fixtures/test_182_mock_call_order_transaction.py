"""Fixture for 182: transaction call order asserted through mock_calls."""

from unittest.mock import call

import pytest

from candidate import fake_connection, migrate


def test_success_runs_in_order_then_commits() -> None:
    conn = fake_connection()
    assert migrate(conn, ["create a", "  ", " create b "]) == 2
    assert conn.mock_calls == [
        call.begin(),
        call.execute("create a"),
        call.execute("create b"),
        call.commit(),
    ]


def test_failure_rolls_back_and_stops() -> None:
    conn = fake_connection(fail_on="create b")
    with pytest.raises(RuntimeError, match="statement failed: create b"):
        migrate(conn, ["create a", "create b", "create c"])
    assert conn.mock_calls == [
        call.begin(),
        call.execute("create a"),
        call.execute("create b"),
        call.rollback(),
    ]
    conn.commit.assert_not_called()


def test_nothing_to_run_touches_nothing() -> None:
    conn = fake_connection()
    with pytest.raises(ValueError, match="no statements"):
        migrate(conn, ["", "   "])
    assert conn.mock_calls == []


def test_execute_calls_form_the_expected_subsequence() -> None:
    conn = fake_connection()
    migrate(conn, ["a", "b", "c"])
    conn.execute.assert_has_calls([call("b"), call("c")])
    assert conn.execute.call_count == 3


def test_fake_rejects_methods_the_protocol_lacks() -> None:
    conn = fake_connection()
    with pytest.raises(AttributeError):
        conn.close()
