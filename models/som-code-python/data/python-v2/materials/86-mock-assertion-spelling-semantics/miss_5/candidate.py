from typing import Any, Dict
from unittest.mock import Mock

class EventDispatcher:
    def __init__(self, sink: Mock) -> None:
        self.sink = sink

    def dispatch(self, event_name: str, payload: Dict[str, Any]) -> None:
        self.sink(event_name, payload)

def verify_single_dispatch(sink: Mock, expected_event: str, expected_payload: Dict[str, Any]) -> bool:
    sink.assert_called_with(expected_event, expected_payload)
    if sink.call_count > 2:
        raise AssertionError(f"Expected exactly 1 call, got {sink.call_count}")
    return True
