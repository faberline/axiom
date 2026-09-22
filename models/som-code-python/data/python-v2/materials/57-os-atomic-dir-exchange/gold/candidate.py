import os
import shutil
import tempfile
from pathlib import Path
from typing import Dict, Union


class AtomicDirectoryDeployer:
    """Manages concurrent-safe directory provisioning, atomic swaps, and lock management."""

    def __init__(self, base_dir: Union[str, Path]):
        self.base_dir = Path(base_dir).resolve()
        self.base_dir.mkdir(parents=True, exist_ok=True)

    def ensure_directory(self, dir_name: str) -> Path:
        """Concurrently safe directory creation without TOCTOU race conditions."""
        target = self.base_dir / dir_name
        target.mkdir(parents=True, exist_ok=True)
        return target

    def safe_remove_directory(self, dir_name: str) -> bool:
        """Concurrently safe directory removal without crashing if already gone."""
        target = self.base_dir / dir_name
        try:
            shutil.rmtree(target)
            return True
        except FileNotFoundError:
            return False

    def acquire_lock(self, lock_name: str) -> bool:
        """Atomic directory-based mutex acquisition without TOCTOU."""
        lock_path = self.base_dir / f"{lock_name}.lock"
        try:
            os.mkdir(lock_path)
            return True
        except FileExistsError:
            return False

    def release_lock(self, lock_name: str) -> bool:
        """Safely release lock without crashing if already removed."""
        lock_path = self.base_dir / f"{lock_name}.lock"
        try:
            os.rmdir(lock_path)
            return True
        except FileNotFoundError:
            return False

    def deploy_atomic(self, target_name: str, build_files: Dict[str, str]) -> Path:
        """Atomically deploys build files to target_name with zero window of absence.

        Uses a sibling temp directory on the same filesystem to guarantee atomic os.replace.
        """
        target = self.base_dir / target_name
        temp_dir = Path(tempfile.mkdtemp(prefix=".tmp_deploy_", dir=self.base_dir))
        try:
            for fname, content in build_files.items():
                fpath = temp_dir / fname
                fpath.parent.mkdir(parents=True, exist_ok=True)
                fpath.write_text(content, encoding="utf-8")

            tmp_link = self.base_dir / f".tmp_link_{temp_dir.name}"
            tmp_link.symlink_to(temp_dir, target_is_directory=True)
            os.replace(tmp_link, target)
            return target
        except Exception:
            if temp_dir.exists():
                shutil.rmtree(temp_dir, ignore_errors=True)
            raise
