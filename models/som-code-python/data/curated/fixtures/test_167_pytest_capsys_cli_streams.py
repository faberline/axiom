import pytest

from candidate import main


@pytest.fixture
def files(tmp_path, monkeypatch):
    monkeypatch.chdir(tmp_path)
    (tmp_path / "a.txt").write_text("one two\nthree\n", encoding="utf-8")
    (tmp_path / "b.txt").write_text("  four   five  ", encoding="utf-8")
    return tmp_path


def test_single_file(files, capsys):
    assert main(["a.txt"]) == 0
    out, err = capsys.readouterr()
    assert out == "     3 a.txt\n"
    assert err == ""


def test_several_files_print_a_total(files, capsys):
    assert main(["a.txt", "b.txt"]) == 0
    assert capsys.readouterr().out == "     3 a.txt\n     2 b.txt\n     5 total\n"


def test_missing_file_goes_to_stderr_and_sets_status(files, capsys):
    assert main(["a.txt", "nope.txt"]) == 1
    out, err = capsys.readouterr()
    assert out == "     3 a.txt\n     3 total\n"
    assert err == "wordcount: nope.txt: no such file\n"


def test_no_arguments_is_a_usage_error(capsys):
    assert main([]) == 2
    out, err = capsys.readouterr()
    assert out == ""
    assert err.startswith("usage: wordcount")


def test_reads_sys_argv_by_default(files, capsys, monkeypatch):
    monkeypatch.setattr("sys.argv", ["wordcount", "b.txt"])
    assert main() == 0
    assert capsys.readouterr().out == "     2 b.txt\n"
