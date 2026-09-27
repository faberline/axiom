import os
import subprocess

import pytest

from candidate import StatusEntry, StatusParseError, git_status, parse_porcelain


def test_plain_records_keep_odd_paths_verbatim():
    data = b" M src/app.py\0?? notes/new line.txt\0A  trailing \0"
    assert parse_porcelain(data) == [
        StatusEntry(" ", "M", "src/app.py"),
        StatusEntry("?", "?", "notes/new line.txt"),
        StatusEntry("A", " ", "trailing "),
    ]


def test_renames_and_copies_consume_the_original_path():
    data = b"R  new.py\0old.py\0C  copy.py\0base.py\0 D gone.py\0"
    assert parse_porcelain(data) == [
        StatusEntry("R", " ", "new.py", "old.py"),
        StatusEntry("C", " ", "copy.py", "base.py"),
        StatusEntry(" ", "D", "gone.py"),
    ]


def test_empty_output_is_clean():
    assert parse_porcelain(b"") == []


def test_malformed_input_raises():
    with pytest.raises(StatusParseError):
        parse_porcelain(b"M\0")
    with pytest.raises(StatusParseError):
        parse_porcelain(b"R  new.py\0")


def git(repo, *args):
    subprocess.run(
        ["git", "-C", str(repo), "-c", "user.name=t", "-c", "user.email=t@t", *args],
        check=True,
        capture_output=True,
    )


def test_git_status_reads_a_real_repository(tmp_path, monkeypatch):
    monkeypatch.setenv("GIT_CONFIG_GLOBAL", os.devnull)
    monkeypatch.setenv("GIT_CONFIG_NOSYSTEM", "1")
    git(tmp_path, "init", "-q")
    (tmp_path / "keep.txt").write_text("a\n")
    (tmp_path / "old.txt").write_text("b\n")
    git(tmp_path, "add", ".")
    git(tmp_path, "commit", "-q", "-m", "init")
    (tmp_path / "keep.txt").write_text("changed\n")
    git(tmp_path, "mv", "old.txt", "renamed.txt")
    (tmp_path / "dir").mkdir()
    (tmp_path / "dir" / "f.txt").write_text("c\n")
    entries = sorted(git_status(tmp_path), key=lambda e: e.path)
    assert entries == [
        StatusEntry("?", "?", "dir/f.txt"),
        StatusEntry(" ", "M", "keep.txt"),
        StatusEntry("R", " ", "renamed.txt", "old.txt"),
    ]
