from typing import Any, Dict, Optional
from unittest.mock import create_autospec, Mock

class UserRepository:
    def find_by_id(self, user_id: int, include_deleted: bool = False) -> Optional[Dict[str, Any]]:
        return {"id": user_id, "name": "Real User"}

    def update_status(self, user_id: int, new_status: str) -> bool:
        return True

def build_user_repo_mock() -> Mock:
    mock_repo = create_autospec(UserRepository, instance=True)
    mock_repo.find_by_id.return_value = {"id": 1, "name": "Mock User"}
    mock_repo.update_status.return_value = True
    return mock_repo

def safe_invoke_repo(repo: Any, method_name: str, *args: Any, **kwargs: Any) -> Any:
    try:
        if callable(repo):
            raise TypeError("Repository must be an instance, not a callable class")
        method = getattr(repo, method_name)
        return method(*args, **kwargs)
    except (TypeError, AttributeError):
        return None
