"""Oracle test suite for 68-playwright-explicit-waits."""
import time
import pytest
import candidate


class MockElement:
    def __init__(self, selector: str, visible: bool = True):
        self.selector = selector
        self.visible = visible
        self.clicked = False

    def click(self):
        if not self.visible:
            raise RuntimeError(f"Element {self.selector} is not visible/interactable")
        self.clicked = True
        return True


class MockPage:
    def __init__(self):
        self.wait_calls = []
        self.query_calls = []

    def wait_for_selector(self, selector: str, state: str = "visible", timeout: float = 30.0):
        self.wait_calls.append({"selector": selector, "state": state, "timeout": timeout})
        return MockElement(selector, visible=(state == "visible"))

    def query_selector(self, selector: str):
        self.query_calls.append(selector)
        return MockElement(selector, visible=False)


def test_explicit_wait_uses_wait_for_selector_not_sleep(monkeypatch):
    """Gold uses page.wait_for_selector(); Miss 1 uses sleep and fails."""
    monkeypatch.setattr(time, "sleep", lambda s: None)
    waiter = candidate.ResilientElementWaiter(default_timeout=5.0)
    page = MockPage()
    elem = waiter.wait_for_element(page, "#target-btn", state="visible")
    assert len(page.wait_calls) == 1, "Must call page.wait_for_selector explicitly"
    assert page.wait_calls[0]["selector"] == "#target-btn"
    assert page.wait_calls[0]["state"] == "visible"
    assert len(page.query_calls) == 0, "Must not fall back to un-synchronized query_selector"
    assert elem is not None


def test_wait_and_click_requires_visible_state(monkeypatch):
    """Gold specifies state='visible'; Miss 2 specifies 'attached' and fails."""
    monkeypatch.setattr(time, "sleep", lambda s: None)
    waiter = candidate.ResilientElementWaiter(default_timeout=5.0)
    page = MockPage()
    elem = waiter.wait_and_click(page, "#submit-btn")
    assert elem.clicked is True
    assert page.wait_calls[0]["state"] == "visible", "wait_and_click must explicitly require 'visible' state"


def test_timeout_and_interval_validation():
    """Gold validates positive numbers; Miss 3 allows <= 0 and fails."""
    with pytest.raises(ValueError, match="positive"):
        candidate.ResilientElementWaiter(default_timeout=0)
    with pytest.raises(ValueError, match="positive"):
        candidate.ResilientElementWaiter(default_timeout=-2.0)
    with pytest.raises(ValueError, match="positive"):
        candidate.ResilientElementWaiter(poll_interval=0)

    waiter = candidate.ResilientElementWaiter()
    page = MockPage()
    with pytest.raises(ValueError, match="positive"):
        waiter.wait_for_element(page, "#btn", timeout=0)


def test_timeout_error_propagated(monkeypatch):
    """Gold propagates TimeoutError; Miss 4 silently swallows and returns None."""
    monkeypatch.setattr(time, "sleep", lambda s: None)
    class FailingPage:
        def wait_for_selector(self, selector, state="visible", timeout=10.0):
            raise TimeoutError(f"Selector {selector} timed out after {timeout}s")

    waiter = candidate.ResilientElementWaiter(default_timeout=2.0)
    with pytest.raises(TimeoutError, match="timed out"):
        waiter.wait_for_element(FailingPage(), "#dynamic-content")


def test_poll_condition_throttles_with_sleep(monkeypatch):
    """Gold sleeps during condition polling; Miss 5 busy-spins and fails."""
    sleep_calls = []
    monkeypatch.setattr(time, "sleep", lambda s: sleep_calls.append(s))

    counter = {"calls": 0}

    def predicate():
        counter["calls"] += 1
        return counter["calls"] >= 3

    waiter = candidate.ResilientElementWaiter(poll_interval=0.01)
    res = waiter.poll_condition(predicate, timeout=1.0)
    assert res is True
    assert len(sleep_calls) >= 2, "poll_condition must sleep between iterations to avoid busy-spinning"
    assert all(s == 0.01 for s in sleep_calls)
