"""Oracle test suite for 86-mock-assertion-spelling-semantics."""
from unittest.mock import Mock
import pytest
import candidate
from candidate import EventDispatcher, verify_single_dispatch


def test_gold_single_dispatch_success():
    """Verify that a single dispatch with expected event and payload succeeds."""
    sink = Mock()
    dispatcher = EventDispatcher(sink)
    dispatcher.dispatch("order_placed", {"order_id": 42, "amount": 99.5})

    assert verify_single_dispatch(sink, "order_placed", {"order_id": 42, "amount": 99.5}) is True


def test_catches_wrong_api_call_or_mismatched_payload():
    """Catches miss_1: non-existent assertion or wrong verification ignores mismatched arguments."""
    sink = Mock()
    dispatcher = EventDispatcher(sink)
    dispatcher.dispatch("order_placed", {"order_id": 42, "amount": 99.5})

    with pytest.raises(AssertionError):
        verify_single_dispatch(sink, "order_placed", {"order_id": 999, "amount": 0.0})


def test_catches_duplicate_dispatch_wrong_default():
    """Catches miss_2: assert_called_with only checks the last call and ignores duplicates."""
    sink = Mock()
    dispatcher = EventDispatcher(sink)
    dispatcher.dispatch("order_placed", {"order_id": 42})
    dispatcher.dispatch("order_placed", {"order_id": 42})

    with pytest.raises(AssertionError):
        verify_single_dispatch(sink, "order_placed", {"order_id": 42})


def test_catches_zero_calls_missing_validation():
    """Catches miss_3: missing invocation () passes even when sink was never called."""
    sink = Mock()
    with pytest.raises(AssertionError):
        verify_single_dispatch(sink, "order_placed", {"order_id": 42})


def test_catches_inverted_call_count_branch():
    """Catches miss_4: inverted call_count check raises on valid single call."""
    sink = Mock()
    dispatcher = EventDispatcher(sink)
    dispatcher.dispatch("user_signup", {"email": "test@example.com"})

    result = verify_single_dispatch(sink, "user_signup", {"email": "test@example.com"})
    assert result is True


def test_catches_excessive_calls_boundary():
    """Catches miss_5: boundary condition allows duplicate dispatches."""
    sink = Mock()
    dispatcher = EventDispatcher(sink)
    dispatcher.dispatch("item_view", {"item_id": 7})
    dispatcher.dispatch("item_view", {"item_id": 7})

    with pytest.raises(AssertionError):
        verify_single_dispatch(sink, "item_view", {"item_id": 7})
