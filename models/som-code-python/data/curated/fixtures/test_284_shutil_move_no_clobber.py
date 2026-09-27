import pytest

import candidate
from candidate import MoveError, free_name, move_all


def make(path, text="x"):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text)
    return path


def test_files_move_into_a_new_nested_inbox(tmp_path):
    src = make(tmp_path / "in" / "a.txt", "alpha")
    inbox = tmp_path / "deep" / "inbox"
    moved = move_all([src], inbox)
    assert moved == [inbox / "a.txt"]
    assert not src.exists()
    assert moved[0].read_text() == "alpha"


def test_collisions_get_numbered_names(tmp_path):
    inbox = tmp_path / "inbox"
    make(inbox / "a.txt", "old")
    make(inbox / "a (1).txt", "older")
    sources = [
        make(tmp_path / "x" / "a.txt", "one"),
        make(tmp_path / "y" / "a.txt", "two"),
    ]
    moved = move_all(sources, inbox)
    assert [p.name for p in moved] == ["a (2).txt", "a (3).txt"]
    assert (inbox / "a.txt").read_text() == "old"
    assert (inbox / "a (3).txt").read_text() == "two"


def test_dangling_symlinks_count_as_taken(tmp_path):
    inbox = tmp_path / "inbox"
    inbox.mkdir()
    (inbox / "b.txt").symlink_to(tmp_path / "gone")
    moved = move_all([make(tmp_path / "b.txt")], inbox)
    assert moved[0].name == "b (1).txt"
    assert (inbox / "b.txt").is_symlink()


def test_only_regular_files_are_moved(tmp_path):
    (tmp_path / "folder").mkdir()
    with pytest.raises(MoveError):
        move_all([tmp_path / "folder"], tmp_path / "inbox")
    with pytest.raises(MoveError):
        move_all([tmp_path / "missing.txt"], tmp_path / "inbox")
    assert (tmp_path / "folder").is_dir()


def test_suffix_limit_is_inclusive(tmp_path, monkeypatch):
    monkeypatch.setattr(candidate, "MAX_SUFFIX", 2)
    make(tmp_path / "a.txt")
    make(tmp_path / "a (1).txt")
    assert free_name(tmp_path, "a.txt").name == "a (2).txt"
    make(tmp_path / "a (2).txt")
    with pytest.raises(MoveError):
        free_name(tmp_path, "a.txt")
