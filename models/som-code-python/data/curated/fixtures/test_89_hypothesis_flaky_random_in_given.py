"""Oracle test suite for 89-hypothesis-flaky-random-in-given."""
import pytest
from hypothesis import find, settings
from hypothesis.errors import NoSuchExample

import candidate
from candidate import (
    encode_token_payload,
    decode_token_payload,
    get_payload_strategy,
    check_roundtrip_invariant,
)


def test_gold_roundtrip_invariant_valid_inputs():
    """Verify that deterministic inputs preserve roundtrip identity."""
    assert check_roundtrip_invariant(0, "admin", 12345) is True
    assert check_roundtrip_invariant(100, "operator", 0) is True
    assert check_roundtrip_invariant(999999, "guest", 65535) is True


def test_catches_flaky_salt_wrong_api_call():
    """Catches miss_1: mutating salt inside test causes invariant mismatch with passed salt."""
    assert check_roundtrip_invariant(42, "auditor", 31415) is True


def test_catches_wrong_default_zero_salt():
    """Catches miss_2: hardcoding salt=0 causes mismatch when caller passes non-zero salt."""
    assert check_roundtrip_invariant(10, "analyst", 4096) is True


def test_catches_missing_validation_role_colon_filter():
    """Catches miss_3: strategy must filter out ':' to prevent token serialization corruption."""
    search = settings(max_examples=2000, database=None, derandomize=True)
    with pytest.raises(NoSuchExample):
        find(get_payload_strategy(), lambda drawn: ":" in drawn[1], settings=search)


def test_catches_wrong_branch_suppressed_value_error():
    """Catches miss_4: catching ValueError and returning True masks validation defects."""
    with pytest.raises(ValueError):
        check_roundtrip_invariant(-1, "admin", 100)


def test_catches_wrong_boundary_zero_user_id():
    """Catches miss_5: rejecting user_id=0 with <= 0 violates valid non-negative boundary."""
    assert check_roundtrip_invariant(0, "root", 500) is True
