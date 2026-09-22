import subprocess
import sys
from typing import List, Optional


class ManagedProcess:
    def __init__(self, cmd: List[str], env: Optional[dict] = None):
        if not isinstance(cmd, list) or not cmd or not all(isinstance(c, str) for c in cmd):
            raise ValueError("cmd must be a non-empty list of strings")
        self.cmd = cmd
        self.args = cmd
        self.env = env
        self.proc: Optional[subprocess.Popen] = None

    def start(self) -> None:
        if self.proc is not None and self.proc.poll() is None:
            raise RuntimeError("Process is already running")
        self.proc = subprocess.Popen(
            self.cmd,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            env=self.env,
        )

    def is_running(self) -> bool:
        if self.proc is None:
            return False
        return self.proc.poll() is None

    def wait(self, timeout: Optional[float] = None) -> int:
        if self.proc is None:
            raise RuntimeError("Process not started")
        return self.proc.wait(timeout=timeout)

    def stop(self, timeout: float = 1.0) -> int:
        if self.proc is None:
            raise RuntimeError("Process not started")
        if timeout <= 0:
            raise ValueError("timeout must be positive")
        if self.proc.poll() is not None:
            return self.proc.returncode

        try:
            self.proc.terminate()
            return 0
        finally:
            if self.proc.stdout and not self.proc.stdout.closed:
                self.proc.stdout.close()
            if self.proc.stderr and not self.proc.stderr.closed:
                self.proc.stderr.close()

    def terminate(self, grace_period: float = 1.0) -> int:
        return self.stop(timeout=grace_period)

    def __enter__(self):
        self.start()
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        if self.is_running():
            self.stop()
