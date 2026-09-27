import pytest

from candidate import NavigationTimeoutError, goto_with_retry


class Page:
    def __init__(self, outcomes):
        self.outcomes = list(outcomes)
        self.calls = []

    def goto(self, url, **kwargs):
        self.calls.append((url, kwargs))
        outcome = self.outcomes.pop(0)
        if isinstance(outcome, Exception):
            raise outcome
        return outcome


def test_first_success_returns_without_sleeping():
    page = Page(["resp"])
    sleeps = []
    assert goto_with_retry(page, "https://x", sleep=sleeps.append) == "resp"
    assert page.calls == [("https://x", {"wait_until": "domcontentloaded"})]
    assert sleeps == []


def test_timeouts_back_off_exponentially():
    page = Page([NavigationTimeoutError(), NavigationTimeoutError(), "ok"])
    sleeps = []
    result = goto_with_retry(page, "u", attempts=3, base_delay=0.5, sleep=sleeps.append)
    assert result == "ok"
    assert sleeps == [0.5, 1.0]


def test_last_timeout_propagates_after_all_attempts():
    page = Page([NavigationTimeoutError()] * 5)
    sleeps = []
    with pytest.raises(NavigationTimeoutError):
        goto_with_retry(page, "u", attempts=3, base_delay=1.0, sleep=sleeps.append)
    assert len(page.calls) == 3
    assert sleeps == [1.0, 2.0]


def test_other_errors_are_not_retried():
    page = Page([ValueError("bad url"), "never"])
    with pytest.raises(ValueError, match="bad url"):
        goto_with_retry(page, "u", sleep=lambda _: None)
    assert len(page.calls) == 1


def test_single_attempt_does_not_retry():
    page = Page([NavigationTimeoutError(), "never"])
    with pytest.raises(NavigationTimeoutError):
        goto_with_retry(page, "u", attempts=1, sleep=lambda _: None)
    assert len(page.calls) == 1


def test_attempts_must_be_positive():
    page = Page(["resp"])
    with pytest.raises(ValueError, match="attempts"):
        goto_with_retry(page, "u", attempts=0)
    assert page.calls == []
