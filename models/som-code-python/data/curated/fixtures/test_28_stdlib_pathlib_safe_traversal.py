from pathlib import Path
import pytest
from candidate import SafeFileSandbox


@pytest.fixture
def sandbox_env(tmp_path):
    root = tmp_path / "sandbox_root"
    root.mkdir()
    sibling = tmp_path / "sandbox_root_secret"
    sibling.mkdir()
    (sibling / "secret.txt").write_text("TOP_SECRET")
    return root, sibling


def test_safe_path_valid_subpaths(sandbox_env):
    root, _ = sandbox_env
    box = SafeFileSandbox(root)
    box.safe_write_text("sub/nested/file.txt", "hello world")
    content = box.safe_read_text("sub/nested/file.txt")
    assert content == "hello world"


def test_sibling_directory_bypass_rejected(sandbox_env):
    root, _ = sandbox_env
    box = SafeFileSandbox(root)
    with pytest.raises(PermissionError):
        box.resolve_safe_path(f"../{root.name}_secret/secret.txt")


def test_leading_slash_treated_relative_to_sandbox(sandbox_env):
    root, _ = sandbox_env
    box = SafeFileSandbox(root)
    (root / "doc.txt").write_text("sandbox content")
    content = box.safe_read_text("/doc.txt")
    assert content == "sandbox content"


def test_symlink_escape_rejected(sandbox_env, tmp_path):
    root, _ = sandbox_env
    outside = tmp_path / "outside"
    outside.mkdir()
    outside_file = outside / "secret.txt"
    outside_file.write_text("ESCAPED")

    link = root / "symlink_dir"
    link.symlink_to(outside, target_is_directory=True)

    box = SafeFileSandbox(root)
    with pytest.raises(PermissionError):
        box.resolve_safe_path("symlink_dir/secret.txt")


def test_dotdot_traversal_rejected(sandbox_env):
    root, _ = sandbox_env
    box = SafeFileSandbox(root)
    with pytest.raises(PermissionError):
        box.resolve_safe_path("sub/../../escaped.txt")


def test_traversal_raises_permission_error_not_root(sandbox_env):
    root, _ = sandbox_env
    box = SafeFileSandbox(root)
    with pytest.raises(PermissionError):
        box.resolve_safe_path("../escaped.txt")
