import os

import pytest

from candidate import scan


@pytest.fixture
def tree(tmp_path):
    for rel in [
        "a.py",
        ".env",
        "notes.txt",
        "pkg/b.py",
        "pkg/.cache.py",
        "pkg/sub/c.py",
        ".git/config",
        "pkg/.hidden/d.py",
    ]:
        path = tmp_path / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text("x")
    return tmp_path


def test_hidden_entries_are_skipped(tree):
    files, errors = scan(str(tree))
    assert files == ["a.py", "notes.txt", "pkg/b.py", "pkg/sub/c.py"]
    assert errors == []


def test_suffix_filter(tree):
    assert scan(str(tree), suffixes=(".py",))[0] == ["a.py", "pkg/b.py", "pkg/sub/c.py"]


def test_max_depth_is_inclusive_of_the_limit(tree):
    assert scan(str(tree), max_depth=0)[0] == ["a.py", "notes.txt"]
    assert scan(str(tree), max_depth=1)[0] == ["a.py", "notes.txt", "pkg/b.py"]
    with pytest.raises(ValueError):
        scan(str(tree), max_depth=-1)


def test_unreadable_directories_are_reported(tree):
    locked = tree / "pkg" / "sub"
    os.chmod(locked, 0)
    try:
        files, errors = scan(str(tree) + "/")
    finally:
        os.chmod(locked, 0o755)
    assert files == ["a.py", "notes.txt", "pkg/b.py"]
    assert errors == ["pkg/sub"]


def test_root_must_be_a_directory(tree):
    with pytest.raises(NotADirectoryError):
        scan(str(tree / "a.py"))
