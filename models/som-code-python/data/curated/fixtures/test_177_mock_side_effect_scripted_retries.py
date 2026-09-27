import pytest

from candidate import call_with_retry, flaky


def test_recovers_after_a_transient_failure():
    service = flaky(ConnectionError("down"), "ok")
    assert call_with_retry(service) == "ok"
    assert service.call_count == 2


def test_gives_up_after_the_last_attempt():
    service = flaky(ConnectionError("1"), ConnectionError("2"), ConnectionError("3"))
    with pytest.raises(ConnectionError, match="3"):
        call_with_retry(service)
    assert service.call_count == 3


def test_other_errors_are_not_retried():
    service = flaky(ValueError("bad input"), "ok")
    with pytest.raises(ValueError, match="bad input"):
        call_with_retry(service)
    assert service.call_count == 1


def test_single_attempt_calls_once():
    service = flaky(ConnectionError("down"), "ok")
    with pytest.raises(ConnectionError):
        call_with_retry(service, attempts=1)
    assert service.call_count == 1


def test_custom_retryable_exceptions():
    service = flaky(TimeoutError(), TimeoutError(), 42)
    assert call_with_retry(service, retry_on=(TimeoutError,)) == 42


def test_attempts_must_be_positive():
    with pytest.raises(ValueError, match="attempts must be at least 1"):
        call_with_retry(flaky("ok"), attempts=0)


def test_flaky_needs_outcomes_and_runs_out():
    with pytest.raises(ValueError, match="at least one outcome"):
        flaky()
    service = flaky("only")
    assert service() == "only"
    with pytest.raises(StopIteration):
        service()
