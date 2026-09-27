"""Verify transaction call order on one spec'd mock through mock_calls."""

from collections.abc import Sequence
from typing import Protocol
from unittest.mock import Mock


class Connection(Protocol):
    """The transactional slice of a database connection."""

    def begin(self) -> None:
        """Open a transaction."""

    def execute(self, sql: str) -> None:
        """Run one statement inside the open transaction."""

    def commit(self) -> None:
        """Make the transaction's changes permanent."""

    def rollback(self) -> None:
        """Discard the transaction's changes."""


def fake_connection(fail_on: str | None = None) -> Mock:
    """A spec'd connection mock whose execute raises RuntimeError for fail_on."""

    def execute(sql: str) -> None:
        if sql == fail_on:
            raise RuntimeError(f"statement failed: {sql}")

    conn = Mock(spec_set=Connection)
    conn.execute.side_effect = execute
    return conn


def migrate(conn: Connection, statements: Sequence[str]) -> int:
    """Run the non-blank statements in one transaction, committing only on success."""
    cleaned = [sql.strip() for sql in statements if sql.strip()]
    if not cleaned:
        raise ValueError("no statements to run")
    conn.begin()
    try:
        for sql in cleaned:
            conn.execute(sql)
    finally:
        conn.commit()
    return len(cleaned)
