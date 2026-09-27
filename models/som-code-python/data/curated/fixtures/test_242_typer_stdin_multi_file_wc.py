import pytest
from typer.testing import CliRunner

from candidate import app

runner = CliRunner()


@pytest.fixture
def files(tmp_path):
    (tmp_path / "a.txt").write_text("one two\nthree\n")
    (tmp_path / "b.txt").write_text("a b\nc")
    return tmp_path


def test_single_file_has_no_total(files):
    result = runner.invoke(app, ["wc", str(files / "a.txt")])
    assert result.exit_code == 0, result.output
    assert result.stdout == f"2\t3\t{files / 'a.txt'}\n"


def test_counts_newlines_not_lines(files):
    result = runner.invoke(app, ["wc", "-l", str(files / "b.txt")])
    assert result.stdout == f"1\t{files / 'b.txt'}\n"


def test_multiple_files_print_a_total(files):
    result = runner.invoke(app, ["wc", str(files / "a.txt"), str(files / "b.txt")])
    assert result.exit_code == 0
    assert result.stdout.splitlines()[-1] == "3\t6\ttotal"


def test_stdin_is_used_without_paths_and_for_dash(files):
    result = runner.invoke(app, ["wc"], input="x y z\n")
    assert result.stdout == "1\t3\t-\n"
    result = runner.invoke(app, ["wc", "-l", "-", str(files / "a.txt")], input="q\n")
    assert result.stdout.splitlines() == ["1\t-", f"2\t{files / 'a.txt'}", "3\ttotal"]


def test_missing_file_is_reported_and_others_still_counted(files):
    missing = files / "nope.txt"
    result = runner.invoke(app, ["wc", str(missing), str(files / "a.txt")])
    assert result.exit_code == 1
    assert f"wc: {missing}: No such file or directory" in result.stderr
    assert "nope" not in result.stdout
    assert result.stdout.splitlines() == [f"2\t3\t{files / 'a.txt'}", "2\t3\ttotal"]
