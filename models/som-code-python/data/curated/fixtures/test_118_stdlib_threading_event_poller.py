import threading
import time

import pytest

from candidate import Poller


def test_task_repeats_until_stopped():
    calls = []
    reached = threading.Event()

    def task():
        calls.append(1)
        if len(calls) >= 3:
            reached.set()

    poller = Poller(task, 0.01)
    poller.start()
    assert reached.wait(1)
    assert poller.stop() is True
    seen = len(calls)
    time.sleep(0.05)
    assert len(calls) == seen


def test_stop_interrupts_a_long_interval():
    ran = threading.Event()
    poller = Poller(ran.set, 10)
    poller.start()
    assert ran.wait(1)
    began = time.monotonic()
    assert poller.stop(timeout=1) is True
    assert time.monotonic() - began < 0.5


def test_task_errors_are_recorded_without_killing_the_thread():
    calls = []
    reached = threading.Event()

    def task():
        calls.append(1)
        if len(calls) <= 2:
            raise OSError(f"flaky {len(calls)}")
        reached.set()

    poller = Poller(task, 0.01)
    poller.start()
    assert reached.wait(1)
    poller.stop()
    assert [str(e) for e in poller.errors[:2]] == ["flaky 1", "flaky 2"]


def test_lifecycle_misuse():
    with pytest.raises(ValueError, match="interval must be positive"):
        Poller(lambda: None, 0)
    poller = Poller(lambda: None, 0.01)
    assert poller.stop() is True
    poller = Poller(lambda: None, 0.01)
    poller.start()
    try:
        with pytest.raises(RuntimeError, match="already started"):
            poller.start()
    finally:
        poller.stop()
