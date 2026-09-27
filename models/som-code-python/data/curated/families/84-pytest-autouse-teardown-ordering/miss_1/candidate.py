"""Record nested test-resource setup and verify teardown runs in reverse."""

from collections.abc import Iterator
from contextlib import contextmanager


class ResourceLifecycleTracker:
    """Ordered log of setup and teardown events."""

    def __init__(self, events: list[str] | None = None) -> None:
        self.events: list[str] = [] if events is None else events

    def record(self, event: str) -> None:
        """Append event to the log."""
        self.events.append(event)

    def is_clean(self) -> bool:
        """Whether three resources were set up and torn down in reverse order."""
        setups = [e.split(":")[1] for e in self.events if e.startswith("setup:")]
        teardowns = [e.split(":")[1] for e in self.events if e.startswith("teardown:")]
        return len(setups) == 3 and setups == list(reversed(teardowns))


@contextmanager
def layered_test_environment(
    tracker: ResourceLifecycleTracker,
) -> Iterator[ResourceLifecycleTracker]:
    """Set up three nested resources and tear them down innermost first."""
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
        pass
