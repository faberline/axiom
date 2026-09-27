import pytest

from candidate import CopyError, copy_project


@pytest.fixture
def project(tmp_path):
    src = tmp_path / "src"
    (src / "pkg" / "__pycache__").mkdir(parents=True)
    (src / ".git").mkdir()
    (src / "pkg" / "app.py").write_text("print(1)")
    (src / "pkg" / "app.pyc").write_bytes(b"\x00")
    (src / "pkg" / "__pycache__" / "app.cpython-312.pyc").write_bytes(b"\x00")
    (src / ".git" / "HEAD").write_text("ref")
    (src / "notes.tmp").write_text("scratch")
    (src / "README.md").write_text("hi")
    (src / "secrets.env").write_text("KEY=1")
    (src / "readme-link.md").symlink_to("README.md")
    return src


def test_ignored_names_are_skipped(project, tmp_path):
    copied = copy_project(project, tmp_path / "build")
    assert copied == [
        "README.md",
        "pkg/app.py",
        "readme-link.md",
        "secrets.env",
    ]


def test_extra_ignores_are_applied(project, tmp_path):
    copied = copy_project(project, tmp_path / "build", ("*.env",))
    assert "secrets.env" not in copied
    assert "README.md" in copied


def test_symlinks_are_copied_as_links(project, tmp_path):
    copy_project(project, tmp_path / "build")
    link = tmp_path / "build" / "readme-link.md"
    assert link.is_symlink()
    assert link.readlink().as_posix() == "README.md"


def test_existing_destination_is_merged(project, tmp_path):
    build = tmp_path / "build"
    build.mkdir()
    (build / "keep.txt").write_text("old")
    copied = copy_project(project, build)
    assert "keep.txt" in copied
    assert (build / "pkg" / "app.py").read_text() == "print(1)"


def test_bad_source_or_destination_raises(project, tmp_path):
    assert issubclass(CopyError, ValueError)
    with pytest.raises(CopyError):
        copy_project(project, project / "build")
    with pytest.raises(CopyError):
        copy_project(project, project)
    with pytest.raises(CopyError):
        copy_project(tmp_path / "missing", tmp_path / "out")
