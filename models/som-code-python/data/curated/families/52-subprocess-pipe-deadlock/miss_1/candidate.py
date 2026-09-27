"""Capture a child's output with communicate() so a full pipe never deadlocks."""

import subprocess
from dataclasses import dataclass


@dataclass
class ProcessOutput:
    """The decoded stdout, stderr, and exit code of a finished process."""

    stdout: str
    stderr: str
    exit_code: int


class SafeOutputCollector:
    """Run a command to completion, draining both pipes under a timeout."""

    def __init__(self, default_timeout: float = 5.0) -> None:
        self.default_timeout = default_timeout

    def run_and_capture(
        self,
        args: list[str],
        input_text: str | None = None,
        timeout: float | None = None,
    ) -> ProcessOutput:
        """Return the output of args, raising when it fails or times out."""
        if (
            not isinstance(args, list)
            or not args
            or not all(isinstance(a, str) for a in args)
        ):
            raise ValueError("args must be a non-empty list of strings")

        effective_timeout = timeout if timeout is not None else self.default_timeout
        if effective_timeout <= 0:
            raise ValueError("timeout must be positive")

        with subprocess.Popen(
            args,
            stdin=subprocess.PIPE if input_text is not None else None,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        ) as proc:
            try:
                proc.wait(timeout=effective_timeout)
                stdout = proc.stdout.read() if proc.stdout else ""
                stderr = proc.stderr.read() if proc.stderr else ""
            except subprocess.TimeoutExpired as exc:
                proc.kill()
                proc.communicate()
                raise TimeoutError(
                    f"Process timed out after {effective_timeout}s"
                ) from exc

        if proc.returncode != 0:
            err_msg = stderr.strip() if stderr else "No error output"
            raise RuntimeError(
                f"Process failed with exit code {proc.returncode}: {err_msg}"
            )

        return ProcessOutput(stdout=stdout, stderr=stderr, exit_code=proc.returncode)


def stream_process(
    cmd: list[str],
    input_data: bytes | None = None,
    timeout: float = 5.0,
) -> tuple[int, bytes, bytes]:
    """Run cmd with byte input and return its exit code and byte output."""
    text_in = input_data.decode("utf-8") if input_data is not None else None
    collector = SafeOutputCollector(default_timeout=timeout)
    res = collector.run_and_capture(cmd, input_text=text_in, timeout=timeout)
    return res.exit_code, res.stdout.encode("utf-8"), res.stderr.encode("utf-8")
