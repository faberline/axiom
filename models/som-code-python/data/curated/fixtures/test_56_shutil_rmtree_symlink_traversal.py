import os
from pathlib import Path
import pytest
from candidate import SafeWorkspaceCleaner


@pytest.fixture
def workspace_setup(tmp_path):
    ws = tmp_path / "workspace"
    ws.mkdir()
    ext = tmp_path / "external_data"
    ext.mkdir()
    (ext / "important.txt").write_text("CRITICAL DATA", encoding="utf-8")
    sibling = tmp_path / "workspace_sibling"
    sibling.mkdir()
    (sibling / "leak.txt").write_text("SIBLING DATA", encoding="utf-8")
    return ws, ext, sibling


def test_clean_valid_file_and_directory(workspace_setup):
    ws, _, _ = workspace_setup
    cleaner = SafeWorkspaceCleaner(ws)
    (ws / "file.txt").write_text("temp", encoding="utf-8")
    sub = ws / "subdir"
    sub.mkdir()
    (sub / "nested.txt").write_text("nested", encoding="utf-8")

    assert cleaner.clean_target("file.txt") is True
    assert not (ws / "file.txt").exists()
    assert cleaner.clean_target("subdir") is True
    assert not sub.exists()


def test_symlink_pointing_to_external_dir_unlinked_without_deleting_target(workspace_setup):
    ws, ext, _ = workspace_setup
    cleaner = SafeWorkspaceCleaner(ws)
    link = ws / "ext_link"
    link.symlink_to(ext, target_is_directory=True)

    # Must remove the symlink itself
    assert cleaner.clean_target("ext_link") is True
    assert not link.exists(follow_symlinks=False)
    # Target files outside workspace MUST NOT be deleted (catches miss_1 and miss_4)
    assert (ext / "important.txt").is_file()
    assert (ext / "important.txt").read_text(encoding="utf-8") == "CRITICAL DATA"


def test_internal_symlink_to_external_not_traversed(workspace_setup):
    ws, ext, _ = workspace_setup
    cleaner = SafeWorkspaceCleaner(ws)
    sub = ws / "build"
    sub.mkdir()
    internal_link = sub / "link_to_ext"
    internal_link.symlink_to(ext, target_is_directory=True)

    assert cleaner.clean_target("build") is True
    assert not sub.exists()
    # Target outside must be preserved (catches miss_4)
    assert (ext / "important.txt").is_file()


def test_parent_escape_raises_permission_error(workspace_setup):
    ws, _, _ = workspace_setup
    cleaner = SafeWorkspaceCleaner(ws)
    # Must raise PermissionError, not return False (catches miss_5)
    with pytest.raises(PermissionError):
        cleaner.clean_target("../outside.txt")


def test_sibling_directory_prefix_bypass_rejected(workspace_setup):
    ws, _, sibling = workspace_setup
    cleaner = SafeWorkspaceCleaner(ws)
    # Must reject path resolving to workspace_sibling (catches miss_2)
    with pytest.raises(PermissionError):
        cleaner.clean_target(f"../{sibling.name}/leak.txt")
    assert (sibling / "leak.txt").is_file()


def test_directory_symlink_unlinked_cleanly(workspace_setup):
    ws, ext, _ = workspace_setup
    cleaner = SafeWorkspaceCleaner(ws)
    dir_symlink = ws / "dir_link"
    dir_symlink.symlink_to(ext, target_is_directory=True)
    # Must unlink without crashing with NotADirectoryError / OSError (catches miss_3)
    assert cleaner.clean_target("dir_link") is True
    assert not dir_symlink.exists(follow_symlinks=False)
