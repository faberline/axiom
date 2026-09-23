import sys
import pytest
from candidate import SafeCommandRunner, CommandResult, execute_command


@pytest.fixture
def runner():
    return SafeCommandRunner(default_timeout=5.0)


def test_valid_command_execution(runner):
    res = runner.run([sys.executable, "-c", "print('hello world')"])
    assert isinstance(res, CommandResult)
    assert res.stdout.strip() == "hello world"
    assert res.stderr == ""
    assert res.exit_code == 0


def test_execute_command_helper():
    res = execute_command(sys.executable, ["-c", "print('direct helper')"])
    assert res.stdout.strip() == "direct helper"
    assert res.exit_code == 0


def test_shell_injection_prevention(runner):
    dangerous_arg = "safe_token; echo INJECTED"
    res = runner.run([sys.executable, "-c", "import sys; print(sys.argv[1])", dangerous_arg])
    assert res.stdout.strip() == dangerous_arg
    assert "INJECTED" not in res.stderr


def test_non_zero_exit_code_raises_runtime_error(runner):
    with pytest.raises(RuntimeError) as exc_info:
        runner.run([sys.executable, "-c", "import sys; sys.exit(42)"])
    assert "42" in str(exc_info.value)


def test_timeout_enforcement(runner):
    with pytest.raises(TimeoutError):
        runner.run([sys.executable, "-c", "import time; time.sleep(2.0)"], timeout=0.1)


def test_timeout_boundary_validation(runner):
    with pytest.raises(ValueError, match="timeout must be positive"):
        runner.run([sys.executable, "-c", "print('fast')"], timeout=0.0)


def test_args_type_validation(runner):
    with pytest.raises(ValueError):
        runner.run("echo not a list")  # type: ignore
    with pytest.raises(ValueError):
        runner.run([])


def test_multiple_arguments_passed_intact(runner):
    res = runner.run([sys.executable, "-c", "import sys; print(len(sys.argv) - 1)", "arg1", "arg2", "arg3"])
    assert res.stdout.strip() == "3"


def test_disallowed_command_raises_permission_error():
    restricted = SafeCommandRunner(allowed_commands=["git"])
    with pytest.raises(PermissionError):
        restricted.run(["docker", "ps"])
    res = SafeCommandRunner(allowed_commands=[sys.executable]).run([sys.executable, "-c", "print('allowed')"])
    assert res.stdout.strip() == "allowed"
