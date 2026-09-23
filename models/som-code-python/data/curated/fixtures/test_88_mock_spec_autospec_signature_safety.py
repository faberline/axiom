"""Oracle test suite for 88-mock-spec-autospec-signature-safety."""
import pytest
import candidate
from candidate import (
    UserRepository,
    build_user_repo_mock,
    safe_invoke_repo,
)


def test_gold_valid_calls_succeed():
    """Verify that valid method calls with matching signatures succeed."""
    repo = build_user_repo_mock()
    user = safe_invoke_repo(repo, "find_by_id", 1, include_deleted=True)
    assert user == {"id": 1, "name": "Mock User"}

    status = safe_invoke_repo(repo, "update_status", 1, new_status="active")
    assert status is True


def test_catches_bare_mock_wrong_default():
    """Catches miss_1: bare Mock without spec allows non-existent methods."""
    repo = build_user_repo_mock()
    with pytest.raises(AttributeError):
        safe_invoke_repo(repo, "delete_all_users")


def test_catches_shallow_spec_wrong_api_call():
    """Catches miss_2: shallow spec=UserRepository does not validate argument signatures."""
    repo = build_user_repo_mock()
    with pytest.raises(TypeError):
        safe_invoke_repo(repo, "find_by_id", 1, invalid_extra_param="hazard")


def test_catches_class_autospec_boundary():
    """Catches miss_3: instance=False creates a callable class mock rather than an instance mock."""
    repo = build_user_repo_mock()
    assert not callable(repo), "Repository mock must be an instance, not a callable class"
    res = safe_invoke_repo(repo, "update_status", 1, "active")
    assert res is True


def test_catches_missing_validation_unbound_method():
    """Catches miss_4: replacing autospecced method with bare Mock allows invalid keywords."""
    repo = build_user_repo_mock()
    with pytest.raises(TypeError):
        safe_invoke_repo(repo, "find_by_id", 1, unvalidated_kwarg=True)


def test_catches_wrong_branch_suppressed_errors():
    """Catches miss_5: catching and suppressing TypeError/AttributeError returns None."""
    repo = build_user_repo_mock()
    with pytest.raises(AttributeError):
        safe_invoke_repo(repo, "non_existent_method")
