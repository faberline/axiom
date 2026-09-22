import os
import shutil
from pathlib import Path
from typing import Union


class SafeWorkspaceCleaner:
    """Safely cleans workspace directories without traversing symlinks or escaping root."""

    def __init__(self, workspace_root: Union[str, Path]):
        self.workspace_root = Path(workspace_root).resolve(strict=True)

    def clean_target(self, target_path: Union[str, Path]) -> bool:
        """Safely remove a file, symlink, or directory within workspace_root.

        Returns True if item was removed, False if it did not exist.
        Raises PermissionError if target_path escapes workspace_root.
        """
        raw = Path(target_path)
        candidate = (self.workspace_root / raw) if not raw.is_absolute() else raw

        # Verify parent directory is valid and contained in workspace_root
        try:
            resolved_parent = candidate.parent.resolve(strict=True)
        except (FileNotFoundError, RuntimeError):
            return False

        if not str(resolved_parent).startswith(str(self.workspace_root)):
            raise PermissionError(f"Target '{target_path}' parent escapes workspace root")

        if candidate.is_symlink():
            candidate.unlink()
            return True

        if not candidate.exists():
            return False

        if candidate.is_file():
            candidate.unlink()
            return True

        # Candidate is a directory: verify resolved path is also within workspace_root
        resolved = candidate.resolve(strict=True)
        if not (resolved == self.workspace_root or resolved.is_relative_to(self.workspace_root)):
            raise PermissionError(f"Target directory '{target_path}' escapes workspace root")

        shutil.rmtree(resolved)
        return True
