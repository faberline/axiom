import asyncio
import os
import sys
import time

import pytest

from candidate import run_all

COUNTER = (
    "import os, sys, time\n"
    "d = sys.argv[1]\n"
    "p = os.path.join(d, str(os.getpid()))\n"
    "open(p, 'w').close()\n"
    "time.sleep(0.4)\n"
    "n = len(os.listdir(d))\n"
    "os.remove(p)\n"
    "print(n)\n"
)


def py(code, *args):
    return [sys.executable, "-c", code, *args]


def max_alive(tmp_path, count, **kwargs):
    commands = [py(COUNTER, str(tmp_path)) for _ in range(count)]
    results = asyncio.run(run_all(commands, **kwargs))
    return max(int(r.stdout) for r in results)


def test_results_follow_input_order():
    commands = [
        py("import time; time.sleep(0.5); print('slow')"),
        py("print('fast')"),
        py("import sys; print('bad'); sys.exit(3)"),
    ]
    results = asyncio.run(run_all(commands))
    assert [r.stdout for r in results] == ["slow\n", "fast\n", "bad\n"]
    assert [r.returncode for r in results] == [0, 0, 3]
    assert results[1].argv == tuple(commands[1])


def test_limit_bounds_alive_processes(tmp_path):
    assert max_alive(tmp_path, 6, limit=2) == 2


def test_default_limit_is_four(tmp_path):
    assert max_alive(tmp_path, 8) == 4


def test_invalid_limit_raises():
    async def bounded():
        await asyncio.wait_for(run_all([py("pass")], limit=0), 3)

    with pytest.raises(ValueError):
        asyncio.run(bounded())


def test_timeout_kills_the_child(tmp_path):
    pid_file = tmp_path / "pid"
    code = (
        "import os, sys, time\n"
        "open(sys.argv[1], 'w').write(str(os.getpid()))\n"
        "time.sleep(5)\n"
    )
    started = time.monotonic()
    with pytest.raises(TimeoutError):
        asyncio.run(run_all([py(code, str(pid_file))], timeout=0.5))
    assert time.monotonic() - started < 4
    pid = int(pid_file.read_text())
    with pytest.raises(ProcessLookupError):
        os.kill(pid, 0)
