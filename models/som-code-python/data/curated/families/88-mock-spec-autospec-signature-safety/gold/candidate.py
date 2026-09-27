"""Build signature-checked repository mocks and refuse to call the class itself."""

from typing import Any
from unittest.mock import Mock, create_autospec


class UserRepository:
    """Repository whose method signatures the mock must enforce."""

    def find_by_id(
        self, user_id: int, include_deleted: bool = False
    ) -> dict[str, Any] | None:
        """Return the user record for user_id."""
        del include_deleted  # Unused: the stub has no soft-deleted rows.
        return {"id": user_id, "name": "Real User"}

    def update_status(self, user_id: int, new_status: str) -> bool:
        """Set the status of user_id and report success."""
        del user_id, new_status  # Unused: the stub stores nothing.
        return True


def build_user_repo_mock() -> Mock:
    """Return an instance autospec of UserRepository with canned results."""
    mock_repo: Mock = create_autospec(UserRepository, instance=True)
    mock_repo.find_by_id.return_value = {"id": 1, "name": "Mock User"}
    mock_repo.update_status.return_value = True
    return mock_repo


def safe_invoke_repo(repo: Any, method_name: str, *args: Any, **kwargs: Any) -> Any:
    """Call method_name on repo, refusing a callable such as the class itself."""
    if callable(repo):
        raise TypeError("Repository must be an instance, not a callable class")
    method = getattr(repo, method_name)
    return method(*args, **kwargs)
