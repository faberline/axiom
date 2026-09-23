import sys
import pytest
from candidate import SafeOutputCollector, ProcessOutput, stream_process


@pytest.fixture
def collector():
    return SafeOutputCollector(default_timeout=3.0)


def test_normal_output_capture(collector):
    py = sys.executable
    res = collector.run_and_capture([py, "-c", "import sys; sys.stdout.write('OUT'); sys.stderr.write('ERR')"])
    assert res.stdout == "OUT"
    assert res.stderr == "ERR"
    assert res.exit_code == 0


def test_stream_process_helper():
    py = sys.executable
    code, out, err = stream_process([py, "-c", "import sys; sys.stdout.write('BYTE_OUT')"])
    assert code == 0
    assert out == b"BYTE_OUT"
    assert err == b""


def test_large_stdout_avoids_pipe_buffer_deadlock(collector):
    script = "import sys; sys.stdout.write('A' * 131072); sys.stdout.flush()"
    res = collector.run_and_capture([sys.executable, "-c", script], timeout=1.0)
    assert len(res.stdout) == 131072
    assert res.exit_code == 0


def test_failing_process_raises_runtime_error_with_stderr(collector):
    script = "import sys; sys.stderr.write('DATABASE_CONNECTION_REFUSED'); sys.exit(7)"
    with pytest.raises(RuntimeError) as exc_info:
        collector.run_and_capture([sys.executable, "-c", script])
    assert "DATABASE_CONNECTION_REFUSED" in str(exc_info.value)
    assert "7" in str(exc_info.value)


def test_timeout_kills_and_reaps_child(collector):
    script = "import time; time.sleep(10.0)"
    with pytest.raises(TimeoutError):
        collector.run_and_capture([sys.executable, "-c", script], timeout=0.1)


def test_timeout_boundary_validation(collector):
    with pytest.raises(ValueError, match="timeout must be positive"):
        collector.run_and_capture([sys.executable, "-c", "print(1)"], timeout=0.0)


def test_stdin_input_piped_successfully(collector):
    script = "import sys; line = sys.stdin.read(); sys.stdout.write(f'ECHO: {line}')"
    res = collector.run_and_capture([sys.executable, "-c", script], input_text="payload_data")
    assert res.stdout == "ECHO: payload_data"


def test_missing_cleanup_reaps_zombie_on_timeout(monkeypatch):
    import subprocess
    orig_kill = subprocess.Popen.kill
    killed_procs = []

    def mock_kill(self):
        killed_procs.append(self)
        return orig_kill(self)

    monkeypatch.setattr(subprocess.Popen, "kill", mock_kill)
    c = SafeOutputCollector(default_timeout=3.0)
    with pytest.raises(TimeoutError):
        c.run_and_capture([sys.executable, "-c", "import time; time.sleep(10.0)"], timeout=0.1)

    assert len(killed_procs) == 1
    assert killed_procs[0].returncode is not None
