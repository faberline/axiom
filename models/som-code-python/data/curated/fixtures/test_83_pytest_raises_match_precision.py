import pytest
from candidate import (
    TransactionError,
    InsufficientFundsError,
    InvalidAccountStatusError,
    process_withdrawal,
    verify_exception_match,
)

def test_valid_withdrawal_success():
    rem = process_withdrawal(100.0, 40.0, "active")
    assert rem == 60.0

def test_regex_pattern_match_verification():
    assert verify_exception_match(
        lambda: process_withdrawal(50.0, 100.0, "active"),
        InsufficientFundsError,
        r"^Insufficient funds: requested \d+\.\d+, balance \d+\.\d+$",
    ) is True

def test_mismatched_message_raises_assertion_error():
    with pytest.raises(AssertionError, match="did not match"):
        verify_exception_match(
            lambda: process_withdrawal(50.0, 100.0, "active"),
            InsufficientFundsError,
            r"^Account status",
        )

def test_wrong_exception_type_raises_assertion_error():
    with pytest.raises(AssertionError, match="Expected InsufficientFundsError, got ValueError"):
        verify_exception_match(
            lambda: process_withdrawal(50.0, -10.0, "active"),
            InsufficientFundsError,
            r"strictly positive",
        )

def test_zero_amount_raises_value_error():
    with pytest.raises(ValueError, match="strictly positive"):
        process_withdrawal(100.0, 0.0, "active")

def test_invalid_status_frozen_raises_invalid_account_status():
    with pytest.raises(InvalidAccountStatusError, match="status 'frozen' is invalid"):
        process_withdrawal(100.0, 10.0, "frozen")
