from typer.testing import CliRunner

from candidate import app

runner = CliRunner()


def test_default_profile():
    result = runner.invoke(app, ["config", "show"], env={"APP_PROFILE": None})
    assert result.exit_code == 0, result.output
    assert result.stdout == "profile: default\n"
    assert result.stderr == ""


def test_profile_option_reaches_nested_command():
    result = runner.invoke(app, ["--profile", "prod", "config", "set", "region", "eu"])
    assert result.exit_code == 0, result.output
    assert result.stdout == "prod.region = eu\n"


def test_profile_from_environment():
    result = runner.invoke(app, ["config", "show"], env={"APP_PROFILE": "staging"})
    assert result.stdout == "profile: staging\n"


def test_verbose_logs_to_stderr_only():
    result = runner.invoke(app, ["-v", "config", "show"], env={"APP_PROFILE": None})
    assert result.exit_code == 0
    assert result.stdout == "profile: default\n"
    assert "[debug] profile=default" in result.stderr


def test_unknown_key_is_a_usage_error():
    result = runner.invoke(app, ["config", "set", "colour", "red"])
    assert result.exit_code == 2
    assert "unknown key 'colour'" in result.stderr
    assert result.stdout == ""
