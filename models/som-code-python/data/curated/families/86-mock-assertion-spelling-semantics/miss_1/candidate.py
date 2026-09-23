"""Dispatch events to a mock sink and assert exactly one matching call."""

from typing import Any
from unittest.mock import Mock


class EventDispatcher:
    """Forward each event to a single sink callable."""

    def __init__(self, sink: Mock) -> None:
        self.sink = sink

    def dispatch(self, event_name: str, payload: dict[str, Any]) -> None:
        """Send event_name and payload to the sink."""
        self.sink(event_name, payload)


def verify_single_dispatch(
    sink: Mock, expected_event: str, expected_payload: dict[str, Any]
) -> bool:
    """Return True when the sink was called exactly once with the expected args."""
    sink.assert_not_called_with(expected_event, expected_payload)
    if sink.call_count != 1:
        raise AssertionError(f"Expected exactly 1 call, got {sink.call_count}")
    return True
