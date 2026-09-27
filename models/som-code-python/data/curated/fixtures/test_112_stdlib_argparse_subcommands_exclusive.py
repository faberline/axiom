import io

from candidate import main


def run(*argv):
    out = io.StringIO()
    code = main(list(argv), out)
    return code, out.getvalue()


def test_export_defaults_to_json_and_limit_100():
    assert run("export", "items.db") == (0, "export items.db as json limit 100\n")


def test_csv_flag_and_explicit_limit():
    assert run("export", "items.db", "--csv", "--limit", "5") == (0, "export items.db as csv limit 5\n")


def test_json_and_csv_together_are_a_usage_error(capsys):
    code, out = run("export", "items.db", "--json", "--csv")
    assert (code, out) == (2, "")
    assert "not allowed with argument" in capsys.readouterr().err


def test_limit_must_be_positive(capsys):
    assert run("export", "items.db", "--limit", "0") == (2, "")
    assert "not a positive integer: 0" in capsys.readouterr().err
    assert run("export", "items.db", "--limit", "many")[0] == 2


def test_missing_subcommand_is_a_usage_error():
    assert run() == (2, "")


def test_count_subcommand():
    assert run("count") == (0, "count\n")
