import subprocess
from dataclasses import dataclass
from typing import List, Optional, Tuple


@dataclass
class ProcessOutput:
    stdout: str
    stderr: str
    exit_code: int


class SafeOutputCollector:
    def __init__(self, default_timeout: float = 5.0):
        self.default_timeout = default_timeout

    def run_and_capture(
        self,
        args: List[str],
        input_text: Optional[str] = None,
        timeout: Optional[float] = None,
    ) -> ProcessOutput:
        if not isinstance(args, list) or not args or not all(isinstance(a, str) for a in args):
            raise ValueError("args must be a non-empty list of strings")

        effective_timeout = timeout if timeout is not None else self.default_timeout
        if effective_timeout <= 0:
            raise ValueError("timeout must be positive")

        proc = subprocess.Popen(
            args,
            stdin=subprocess.PIPE if input_text is not None else None,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
        try:
            stdout, stderr = proc.communicate(input=input_text, timeout=effective_timeout)
        except subprocess.TimeoutExpired as exc:
            proc.kill()
            raise TimeoutError(f"Process timed out after {effective_timeout}s") from exc

        if proc.returncode != 0:
            err_msg = stderr.strip() if stderr else "No error output"
            raise RuntimeError(f"Process failed with exit code {proc.returncode}: {err_msg}")

        return ProcessOutput(stdout=stdout, stderr=stderr, exit_code=proc.returncode)


def stream_process(
    cmd: List[str],
    input_data: Optional[bytes] = None,
    timeout: float = 5.0,
) -> Tuple[int, bytes, bytes]:
    text_in = input_data.decode("utf-8") if input_data is not None else None
    collector = SafeOutputCollector(default_timeout=timeout)
    res = collector.run_and_capture(cmd, input_text=text_in, timeout=timeout)
    return res.exit_code, res.stdout.encode("utf-8"), res.stderr.encode("utf-8")
