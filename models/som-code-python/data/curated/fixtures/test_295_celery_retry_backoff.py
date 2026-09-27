import pytest

import candidate
from candidate import MAX_RETRIES, PermanentError, TransientError, backoff, deliver


def scripted(monkeypatch, outcomes):
    calls = []

    def fake(url, payload):
        calls.append((url, payload))
        outcome = outcomes[len(calls) - 1]
        if isinstance(outcome, Exception):
            raise outcome
        return outcome

    monkeypatch.setattr(candidate, "send_webhook", fake)
    return calls


def test_transient_failures_are_retried_until_success(monkeypatch):
    calls = scripted(monkeypatch, [TransientError("t1"), TransientError("t2"), 202])
    result = deliver.apply(args=("https://hook.test", {"id": 1}))
    assert result.get() == 202
    assert len(calls) == 3


def test_retries_stop_at_the_limit(monkeypatch):
    calls = scripted(monkeypatch, [TransientError(str(i)) for i in range(10)])
    with pytest.raises(TransientError):
        deliver.apply(args=("https://hook.test", {})).get()
    assert len(calls) == MAX_RETRIES + 1 == 5


def test_permanent_errors_are_not_retried(monkeypatch):
    calls = scripted(monkeypatch, [PermanentError("400"), 200])
    with pytest.raises(PermanentError):
        deliver.apply(args=("https://hook.test", {})).get()
    assert len(calls) == 1


def test_backoff_doubles_and_caps():
    ceiling = [backoff(n, lambda low, high: high) for n in range(7)]
    assert ceiling == [2.0, 4.0, 8.0, 16.0, 32.0, 60.0, 60.0]
    assert backoff(3, lambda low, high: low) == 0.0
    with pytest.raises(ValueError):
        backoff(-1)


def test_task_options():
    assert deliver.acks_late is True
    assert deliver.max_retries == 4
    for n in range(10):
        assert 0.0 <= backoff(n) <= 60.0
