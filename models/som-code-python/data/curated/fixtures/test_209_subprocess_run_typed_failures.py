import inspect
import sys
import time

import pytest

from candidate import CommandError, CommandTimeoutError, run_command

PY = sys.executable


def test_stdout_is_returned():
    assert run_command([PY, "-c", "print('hello')"]) == "hello\n"


def test_failure_keeps_the_stderr_tail():
    script = "import sys; sys.stderr.write('a' * 100 + 'b' * 500); sys.exit(3)"
    with pytest.raises(CommandError) as info:
        run_command([PY, "-c", script])
    assert info.value.returncode == 3
    assert info.value.stderr == "b" * 500
    assert str(info.value).startswith(f"{PY} exited with 3: b")


def test_death_by_signal_is_a_failure():
    script = "import os, signal; os.kill(os.getpid(), signal.SIGTERM)"
    with pytest.raises(CommandError) as info:
        run_command([PY, "-c", script])
    assert info.value.returncode < 0


def test_timeout_raises_typed_error_quickly():
    started = time.monotonic()
    with pytest.raises(CommandTimeoutError, match="timed out after 0.5s") as info:
        run_command([PY, "-c", "import time; time.sleep(10)"], timeout=0.5)
    assert info.value.timeout == 0.5
    assert time.monotonic() - started < 5


def test_empty_argv_and_default_timeout():
    with pytest.raises(ValueError, match="must not be empty"):
        run_command([])
    assert inspect.signature(run_command).parameters["timeout"].default == 30.0
