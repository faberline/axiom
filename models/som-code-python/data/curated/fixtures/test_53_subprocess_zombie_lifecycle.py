import sys
import time
import pytest
from candidate import ManagedProcess


def test_normal_stop_reaps_process():
    py = sys.executable
    p = ManagedProcess([py, "-c", "import time; time.sleep(10.0)"])
    p.start()
    assert p.is_running() is True
    exit_code = p.stop(timeout=0.5)
    assert p.proc.returncode is not None, "Child process must be reaped"
    assert p.is_running() is False


def test_kill_unresponsive_process_reaps_zombie():
    script = """import signal, time, sys
signal.signal(signal.SIGTERM, signal.SIG_IGN)
print('READY', flush=True)
time.sleep(10.0)
"""
    p = ManagedProcess([sys.executable, "-c", script])
    p.start()
    line = p.proc.stdout.readline()
    assert line.strip() == "READY"

    ret = p.stop(timeout=0.2)
    assert p.proc.returncode is not None, "Killed process must be reaped"
    assert ret == p.proc.returncode


def test_context_manager_cleans_up_on_exception():
    p_ref = None
    try:
        with ManagedProcess([sys.executable, "-c", "import time; time.sleep(10.0)"]) as p:
            p_ref = p
            raise RuntimeError("simulated failure")
    except RuntimeError:
        pass

    assert p_ref is not None
    assert p_ref.proc.poll() is not None, "Context manager must stop and reap child"


def test_is_running_polls_and_reaps_finished_child():
    p = ManagedProcess([sys.executable, "-c", "import sys; sys.exit(0)"])
    p.start()
    time.sleep(0.1)
    assert p.is_running() is False
    assert p.proc.returncode == 0


def test_pipes_closed_on_stop():
    p = ManagedProcess([sys.executable, "-c", "import time; time.sleep(10.0)"])
    p.start()
    p.stop(timeout=0.5)
    assert p.proc.stdout.closed is True
    assert p.proc.stderr.closed is True


def test_stop_boundary_validation():
    p = ManagedProcess([sys.executable, "-c", "import time; time.sleep(10.0)"])
    p.start()
    try:
        with pytest.raises(ValueError, match="timeout must be positive"):
            p.stop(timeout=0.0)
    finally:
        p.stop(timeout=0.5)


def test_cmd_validation():
    with pytest.raises(ValueError):
        ManagedProcess([])
    with pytest.raises(ValueError):
        ManagedProcess("not a list")  # type: ignore
