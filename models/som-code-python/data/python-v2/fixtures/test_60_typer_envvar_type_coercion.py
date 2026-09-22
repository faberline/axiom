import pytest
from typer.testing import CliRunner
from candidate import app

runner = CliRunner()


def test_serve_default_configuration():
    result = runner.invoke(app, ["serve"])
    assert result.exit_code == 0
    # Verifies metrics=True by default (catches miss_4)
    assert "CONFIG: workers=4 timeout=30.0 metrics=True endpoints=0" in result.stdout


def test_endpoints_envvar_parsing_cleans_whitespace_and_empty():
    # Endpoints with whitespace and trailing comma must be cleanly parsed (catches miss_1)
    result = runner.invoke(app, ["serve"], env={"GATEWAY_ENDPOINTS": "api/v1,  api/v2 , "})
    assert result.exit_code == 0
    assert "endpoints=2" in result.stdout


def test_workers_zero_rejected_with_exit_code_2():
    # Non-positive workers must fail with exit code 2 (catches miss_2)
    result = runner.invoke(app, ["serve"], env={"GATEWAY_WORKERS": "0"})
    assert result.exit_code == 2
    assert "workers must be >= 1" in result.stderr


def test_timeout_zero_rejected_with_exit_code_2():
    # Timeout 0.0 must be rejected with exit code 2 (catches miss_3)
    result = runner.invoke(app, ["serve"], env={"GATEWAY_TIMEOUT": "0.0"})
    assert result.exit_code == 2
    assert "timeout must be > 0" in result.stderr


def test_timeout_envvar_override():
    # GATEWAY_TIMEOUT="45.5" must update timeout (catches miss_5)
    result = runner.invoke(app, ["serve"], env={"GATEWAY_TIMEOUT": "45.5"})
    assert result.exit_code == 0
    assert "timeout=45.5" in result.stdout
