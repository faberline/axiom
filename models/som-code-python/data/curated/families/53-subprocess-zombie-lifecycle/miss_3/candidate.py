"""Supervise one background child process and always reap what it starts."""

import subprocess
from types import TracebackType
from typing import Self


class ManagedProcess:
    """Start, poll, stop, and reap a single child process."""

    def __init__(self, cmd: list[str], env: dict[str, str] | None = None) -> None:
        if (
            not isinstance(cmd, list)
            or not cmd
            or not all(isinstance(c, str) for c in cmd)
        ):
            raise ValueError("cmd must be a non-empty list of strings")
        self.cmd = cmd
        self.args = cmd
        self.env = env
        self.proc: subprocess.Popen[str] | None = None

    def start(self) -> None:
        """Launch the command, refusing while a previous child still runs."""
        if self.proc is not None and self.proc.poll() is None:
            raise RuntimeError("Process is already running")
        # The child outlives start(); stop() and __exit__ reap it.
        self.proc = subprocess.Popen(  # pylint: disable=consider-using-with
            self.cmd,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            env=self.env,
        )

    def is_running(self) -> bool:
        """Return whether the child has started and not yet exited."""
        if self.proc is None:
            return False
        return self.proc.poll() is None

    def wait(self, timeout: float | None = None) -> int:
        """Block until the child exits and return its exit code."""
        if self.proc is None:
            raise RuntimeError("Process not started")
        return self.proc.wait(timeout=timeout)

    def stop(self, timeout: float = 1.0) -> int:
        """Terminate the child, kill it after timeout, and return its exit code."""
        if self.proc is None:
            raise RuntimeError("Process not started")
        if timeout <= 0:
            raise ValueError("timeout must be positive")
        if self.proc.poll() is not None:
            return self.proc.returncode

        try:
            self.proc.terminate()
            try:
                return self.proc.wait(timeout=timeout)
            except subprocess.TimeoutExpired:
                self.proc.kill()
                return self.proc.wait(timeout=timeout)
        finally:
            if self.proc.stdout and not self.proc.stdout.closed:
                self.proc.stdout.close()
            if self.proc.stderr and not self.proc.stderr.closed:
                self.proc.stderr.close()

    def terminate(self, grace_period: float = 1.0) -> int:
        """Stop the child with grace_period seconds before a kill."""
        return self.stop(timeout=grace_period)

    def __enter__(self) -> Self:
        self.start()
        return self

    def __exit__(
        self,
        exc_type: type[BaseException] | None,
        exc_val: BaseException | None,
        exc_tb: TracebackType | None,
    ) -> None:
        if not self.is_running():
            self.stop()
