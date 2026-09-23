from click.testing import CliRunner
import pytest
from candidate import cli

runner = CliRunner()


def test_cli_flags_default_values():
    result = runner.invoke(cli, ["sync"])
    assert result.exit_code == 0
    # Verifies force=False by default (catches miss_2)
    assert "STATUS: dry_run=False force=False retries=3" in result.stdout


def test_envvar_boolean_false_strings_parsed_as_false():
    # "false", "0", "no" must all evaluate to False (catches miss_1)
    for false_str in ("false", "0", "no"):
        result = runner.invoke(cli, ["sync"], env={"SYNC_DRY_RUN": false_str})
        assert result.exit_code == 0
        assert "dry_run=False" in result.stdout


def test_envvar_boolean_true_strings_parsed_as_true():
    # "true", "1", "yes" must evaluate to True (catches miss_3)
    result = runner.invoke(cli, ["sync"], env={"SYNC_DRY_RUN": "true", "SYNC_FORCE": "1"})
    assert result.exit_code == 0
    assert "dry_run=True" in result.stdout
    assert "force=True" in result.stdout


def test_envvar_invalid_boolean_raises_bad_parameter_exit_code_2():
    # Malformed boolean string must fail validation with exit code 2 (catches miss_5)
    result = runner.invoke(cli, ["sync"], env={"SYNC_DRY_RUN": "not_a_boolean"})
    assert result.exit_code == 2


def test_envvar_retries_out_of_range_rejected():
    # Negative retries must be rejected with exit code 2 (catches miss_4)
    result = runner.invoke(cli, ["sync"], env={"SYNC_RETRIES": "-2"})
    assert result.exit_code == 2
