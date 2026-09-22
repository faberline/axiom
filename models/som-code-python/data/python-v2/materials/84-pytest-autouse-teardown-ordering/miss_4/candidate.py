from typing import List, Optional
from contextlib import contextmanager

_SHARED_EVENTS: List[str] = []

class ResourceLifecycleTracker:
    def __init__(self, events: List[str] = _SHARED_EVENTS) -> None:
        self.events: List[str] = events

    def record(self, event: str) -> None:
        self.events.append(event)

    def is_clean(self) -> bool:
        setups = [e.split(":")[1] for e in self.events if e.startswith("setup:")]
        teardowns = [e.split(":")[1] for e in self.events if e.startswith("teardown:")]
        return len(setups) == 3 and setups == list(reversed(teardowns))

@contextmanager
def layered_test_environment(tracker: ResourceLifecycleTracker):
    tracker.record("setup:lock_manager")
    try:
        tracker.record("setup:db_session")
        try:
            tracker.record("setup:audit_recorder")
            try:
                yield tracker
            finally:
                tracker.record("teardown:audit_recorder")
        finally:
            tracker.record("teardown:db_session")
    finally:
        tracker.record("teardown:lock_manager")
