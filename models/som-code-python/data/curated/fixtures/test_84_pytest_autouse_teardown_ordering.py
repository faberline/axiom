import pytest
from candidate import ResourceLifecycleTracker, layered_test_environment

def test_lifo_teardown_normal_execution():
    tracker = ResourceLifecycleTracker()
    with layered_test_environment(tracker):
        tracker.record("test:executing")
    expected = [
        "setup:lock_manager",
        "setup:db_session",
        "setup:audit_recorder",
        "test:executing",
        "teardown:audit_recorder",
        "teardown:db_session",
        "teardown:lock_manager",
    ]
    assert tracker.events == expected
    assert tracker.is_clean() is True

def test_lifo_teardown_guaranteed_on_exception():
    tracker = ResourceLifecycleTracker()
    with pytest.raises(ZeroDivisionError):
        with layered_test_environment(tracker):
            _ = 1 / 0
    assert tracker.is_clean() is True
    teardowns = [e for e in tracker.events if e.startswith("teardown:")]
    assert teardowns == ["teardown:audit_recorder", "teardown:db_session", "teardown:lock_manager"]

def test_independent_tracker_instances():
    t1 = ResourceLifecycleTracker()
    t1.record("event1")
    t2 = ResourceLifecycleTracker()
    assert len(t2.events) == 0
