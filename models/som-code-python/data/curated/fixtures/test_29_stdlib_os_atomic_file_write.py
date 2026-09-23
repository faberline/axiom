import os
from pathlib import Path
import pytest
from candidate import AtomicFileOps


def test_atomic_write_success(tmp_path):
    target = tmp_path / "test.txt"
    AtomicFileOps.write_text_atomic(target, "atomic content")
    assert target.is_file()
    assert target.read_text(encoding="utf-8") == "atomic content"

    json_target = tmp_path / "test.json"
    AtomicFileOps.write_json_atomic(json_target, {"key": "value"})
    assert json_target.is_file()
    assert '"key": "value"' in json_target.read_text(encoding="utf-8")


def test_original_file_preserved_on_write_failure(tmp_path, monkeypatch):
    target = tmp_path / "original.txt"
    target.write_text("PRESERVED_CONTENT", encoding="utf-8")

    def failing_write(self, s):
        raise IOError("Disk full simulation")

    monkeypatch.setattr("tempfile._TemporaryFileWrapper.write", failing_write, raising=False)
    with pytest.raises(Exception):
        AtomicFileOps.write_text_atomic(target, "CORRUPTED_CONTENT")

    assert target.read_text(encoding="utf-8") == "PRESERVED_CONTENT"


def test_temp_file_created_in_same_parent_directory(tmp_path, monkeypatch):
    target = tmp_path / "subdir" / "target.txt"
    target.parent.mkdir(parents=True, exist_ok=True)
    created_dirs = []

    orig_ntf = AtomicFileOps.write_text_atomic.__globals__["tempfile"].NamedTemporaryFile

    def spy_ntf(*args, **kwargs):
        created_dirs.append(kwargs.get("dir"))
        return orig_ntf(*args, **kwargs)

    monkeypatch.setattr(AtomicFileOps.write_text_atomic.__globals__["tempfile"], "NamedTemporaryFile", spy_ntf)
    AtomicFileOps.write_text_atomic(target, "content")
    assert len(created_dirs) == 1
    assert created_dirs[0] == target.parent


def test_fsync_called_before_replace(tmp_path, monkeypatch):
    target = tmp_path / "fsync_test.txt"
    fsync_called = False

    orig_fsync = os.fsync

    def monitored_fsync(fd):
        nonlocal fsync_called
        fsync_called = True
        return orig_fsync(fd)

    monkeypatch.setattr(os, "fsync", monitored_fsync)
    AtomicFileOps.write_text_atomic(target, "durability test")
    assert fsync_called is True


def test_temp_file_cleaned_up_on_failure(tmp_path, monkeypatch):
    target = tmp_path / "cleanup_target.txt"

    def failing_fsync(fd):
        raise OSError("Fsync failure")

    monkeypatch.setattr(os, "fsync", failing_fsync)
    with pytest.raises(OSError):
        AtomicFileOps.write_text_atomic(target, "fail content")

    leftovers = list(tmp_path.glob("tmp*"))
    assert len(leftovers) == 0, f"Found leaked temp files: {leftovers}"


def test_atomic_replace_called(tmp_path, monkeypatch):
    target = tmp_path / "replace_target.txt"
    replace_called = False

    orig_replace = os.replace

    def monitored_replace(src, dst):
        nonlocal replace_called
        replace_called = True
        return orig_replace(src, dst)

    monkeypatch.setattr(os, "replace", monitored_replace)
    AtomicFileOps.write_text_atomic(target, "replace content")
    assert replace_called is True
