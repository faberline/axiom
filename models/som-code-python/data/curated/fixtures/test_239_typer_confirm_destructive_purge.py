import pytest
from typer.testing import CliRunner

from candidate import app

runner = CliRunner()


@pytest.fixture
def snaps(tmp_path):
    for name in ("a.snap", "b.snap", "keep.txt"):
        (tmp_path / name).write_text("x")
    (tmp_path / "dir.snap").mkdir()
    return tmp_path


def remaining(path):
    return sorted(p.name for p in path.iterdir())


def test_confirmed_purge_deletes_only_matching_files(snaps):
    result = runner.invoke(app, ["purge", str(snaps)], input="y\n")
    assert result.exit_code == 0, result.output
    assert "a.snap\nb.snap\n" in result.output
    assert "deleted 2 files" in result.output
    assert remaining(snaps) == ["dir.snap", "keep.txt"]


def test_declined_prompt_aborts_without_deleting(snaps):
    result = runner.invoke(app, ["purge", str(snaps)], input="n\n")
    assert result.exit_code == 1
    assert "Aborted" in result.output
    assert remaining(snaps) == ["a.snap", "b.snap", "dir.snap", "keep.txt"]


def test_yes_flag_skips_the_prompt(snaps):
    result = runner.invoke(app, ["purge", str(snaps), "-y"])
    assert result.exit_code == 0, result.output
    assert "Delete 2 files?" not in result.output
    assert remaining(snaps) == ["dir.snap", "keep.txt"]


def test_dry_run_deletes_nothing(snaps):
    result = runner.invoke(app, ["purge", str(snaps), "--dry-run", "--yes"])
    assert result.exit_code == 0
    assert "would delete 2 files" in result.output
    assert remaining(snaps) == ["a.snap", "b.snap", "dir.snap", "keep.txt"]


def test_no_match_exits_zero(snaps):
    result = runner.invoke(app, ["purge", str(snaps), "--pattern", "*.bak"])
    assert result.exit_code == 0
    assert "nothing to purge" in result.output


def test_directory_must_exist_and_be_a_directory(snaps):
    assert runner.invoke(app, ["purge", str(snaps / "missing")]).exit_code == 2
    assert runner.invoke(app, ["purge", str(snaps / "keep.txt")]).exit_code == 2
