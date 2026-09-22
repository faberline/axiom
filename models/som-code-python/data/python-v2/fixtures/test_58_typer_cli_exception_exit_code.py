from pathlib import Path
import pytest
from typer.testing import CliRunner
from candidate import app

runner = CliRunner()


def test_deploy_success_exits_zero(tmp_path):
    pkg = tmp_path / "app.tar.gz"
    pkg.write_text("VALID_PACKAGE", encoding="utf-8")

    result = runner.invoke(app, ["deploy", "--artifact", str(pkg), "--env", "staging"])
    assert result.exit_code == 0
    assert "SUCCESS: app.tar.gz deployed to staging" in result.stdout


def test_missing_artifact_exits_code_1_and_logs_to_stderr(tmp_path):
    missing_pkg = tmp_path / "missing.tar.gz"
    result = runner.invoke(app, ["deploy", "--artifact", str(missing_pkg)])

    # Must exit code 1 (catches miss_1 and miss_2)
    assert result.exit_code == 1
    # Error message must be in stderr, not stdout (catches miss_5)
    assert "Error: Artifact not found" in result.stderr
    assert "Error: Artifact not found" not in result.stdout


def test_invalid_env_exits_code_2(tmp_path):
    pkg = tmp_path / "app.tar.gz"
    pkg.write_text("VALID", encoding="utf-8")

    result = runner.invoke(app, ["deploy", "--artifact", str(pkg), "--env", "development"])
    # Must exit code 2 (catches miss_4)
    assert result.exit_code == 2
    assert "Validation Error" in result.stderr


def test_empty_artifact_exits_code_3_without_raw_traceback(tmp_path):
    empty_pkg = tmp_path / "empty.tar.gz"
    empty_pkg.write_text("", encoding="utf-8")

    result = runner.invoke(app, ["deploy", "--artifact", str(empty_pkg)])
    # Must exit code 3 (catches miss_3)
    assert result.exit_code == 3
    assert "Deployment Failed: Artifact payload is empty" in result.stderr
    assert "Traceback (most recent call last)" not in result.stderr
