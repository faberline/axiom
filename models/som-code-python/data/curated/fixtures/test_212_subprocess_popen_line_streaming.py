import inspect
import sys
import time

import pytest

from candidate import StreamResult, stream_command

MIXED = (
    "import sys\n"
    "print('  indented', flush=True)\n"
    "print('oops', file=sys.stderr, flush=True)\n"
    "print('done', flush=True)\n"
    "sys.exit(3)"
)
MANY = "for i in range(30): print(i)"
FOREVER = "import itertools\nfor i in itertools.count(): print(i, flush=True)"


def test_stdout_and_stderr_arrive_in_order():
    seen = []
    result = stream_command([sys.executable, "-c", MIXED], seen.append)
    assert seen == ["  indented", "oops", "done"]
    assert result == StreamResult(3, ["  indented", "oops", "done"])


def test_tail_keeps_exactly_the_last_lines():
    result = stream_command(
        [sys.executable, "-c", MANY], lambda line: None, tail_lines=5
    )
    assert result.tail == ["25", "26", "27", "28", "29"]
    assert result.returncode == 0


def test_default_tail_is_twenty_lines():
    assert inspect.signature(stream_command).parameters["tail_lines"].default == 20
    result = stream_command([sys.executable, "-c", MANY], lambda line: None)
    assert result.tail == [str(i) for i in range(10, 30)]


def test_invalid_tail_raises():
    with pytest.raises(ValueError, match="at least 1"):
        stream_command([sys.executable, "-c", "pass"], print, tail_lines=0)


def test_callback_error_kills_the_process():
    def stop(line):
        if line == "3":
            raise KeyboardInterrupt

    started = time.monotonic()
    with pytest.raises(KeyboardInterrupt):
        stream_command([sys.executable, "-c", FOREVER], stop)
    assert time.monotonic() - started < 10
