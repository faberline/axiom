import os
import shutil
import tempfile
from pathlib import Path
import pytest
from candidate import AtomicDirectoryDeployer


@pytest.fixture
def deployer(tmp_path):
    return AtomicDirectoryDeployer(tmp_path / "deploy_root")


def test_ensure_directory_race_resilient(deployer, monkeypatch):
    # Verify that ensure_directory does not crash when directory already exists (catches miss_1)
    target = deployer.base_dir / "shared_dir"
    orig_exists = Path.exists

    def race_exists(p):
        if p == target:
            # Simulate competitor creating directory right after check
            target.mkdir(parents=True, exist_ok=True)
            return False
        return orig_exists(p)

    monkeypatch.setattr(Path, "exists", race_exists)
    d = deployer.ensure_directory("shared_dir")
    assert d.is_dir()


def test_safe_remove_directory_race_resilient(deployer, monkeypatch):
    target = deployer.base_dir / "toctou_dir"
    target.mkdir()

    def vanishing_rmtree(p, *args, **kwargs):
        raise FileNotFoundError(f"No such directory: {p}")

    monkeypatch.setattr(shutil, "rmtree", vanishing_rmtree)
    # Gold catches FileNotFoundError and returns False safely; miss_2 crashes (catches miss_2)
    assert deployer.safe_remove_directory("toctou_dir") is False


def test_acquire_lock_race_resilient(deployer, monkeypatch):
    target_lock = deployer.base_dir / "concurrent.lock"
    called_race = False
    orig_exists = Path.exists

    def race_exists(p):
        nonlocal called_race
        if p == target_lock:
            called_race = True
            # Simulate competitor acquiring lock right before mkdir
            target_lock.mkdir(exist_ok=True)
            return False
        return orig_exists(p)

    monkeypatch.setattr(Path, "exists", race_exists)
    res = deployer.acquire_lock("concurrent")
    if not called_race:
        assert res is True
        # Lock is held; second acquisition must return False
        assert deployer.acquire_lock("concurrent") is False
    else:
        # If exists() was called, it must have handled FileExistsError
        assert res is False
    assert deployer.release_lock("concurrent") is True


def test_deploy_atomic_zero_window_of_absence(deployer, monkeypatch):
    # Initial deployment
    deployer.deploy_atomic("current", {"app.py": "v1"})
    target = deployer.base_dir / "current"
    assert (target / "app.py").read_text(encoding="utf-8") == "v1"

    # In atomic deployment, target symlink must NEVER be removed prior to replacement
    unlink_called_on_target = False
    orig_unlink = Path.unlink

    def spy_unlink(path_obj, *args, **kwargs):
        nonlocal unlink_called_on_target
        if path_obj == target:
            unlink_called_on_target = True
        return orig_unlink(path_obj, *args, **kwargs)

    monkeypatch.setattr(Path, "unlink", spy_unlink)
    deployer.deploy_atomic("current", {"app.py": "v2"})
    # Atomic deploy must not unlink target before replacement (catches miss_4)
    assert unlink_called_on_target is False
    assert (target / "app.py").read_text(encoding="utf-8") == "v2"


def test_deploy_atomic_uses_sibling_directory(deployer, monkeypatch):
    orig_mkdtemp = tempfile.mkdtemp
    captured_dirs = []

    def mock_mkdtemp(*args, **kwargs):
        captured_dirs.append(kwargs.get("dir"))
        return orig_mkdtemp(*args, **kwargs)

    monkeypatch.setattr(tempfile, "mkdtemp", mock_mkdtemp)
    deployer.deploy_atomic("app_prod", {"index.html": "<h1>Hello</h1>"})

    # Temporary directory must be created in base_dir for same-filesystem atomicity (catches miss_5)
    assert len(captured_dirs) == 1
    assert captured_dirs[0] == deployer.base_dir
