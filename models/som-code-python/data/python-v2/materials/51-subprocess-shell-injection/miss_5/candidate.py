import subprocess
import sys
from dataclasses import dataclass
from typing import List, Optional, Set


@dataclass
class CommandResult:
    stdout: str
    stderr: str
    exit_code: int


ExecutionResult = CommandResult


class SafeCommandRunner:
    def __init__(self, allowed_commands: Optional[List[str]] = None, default_timeout: float = 10.0):
        self.allowed_commands: Optional[Set[str]] = set(allowed_commands) if allowed_commands is not None else None
        self.default_timeout = default_timeout

    def run(self, args: List[str], timeout: Optional[float] = None) -> CommandResult:
        if not isinstance(args, list) or not args or not all(isinstance(a, str) for a in args):
            raise ValueError("args must be a non-empty list of strings")

        cmd = args[0]
        if self.allowed_commands is not None and cmd not in self.allowed_commands:
            raise PermissionError(f"Command '{cmd}' is not in allowed commands")

        effective_timeout = timeout if timeout is not None else self.default_timeout
        if effective_timeout <= 0:
            raise ValueError("timeout must be positive")

        try:
            proc = subprocess.run(
                args,
                capture_output=True,
                text=True,
                shell=True,
                check=True,
                timeout=effective_timeout,
            )
            return CommandResult(stdout=proc.stdout, stderr=proc.stderr, exit_code=proc.returncode)
        except subprocess.CalledProcessError as exc:
            raise RuntimeError(f"Command failed with exit code {exc.returncode}: {exc.stderr}") from exc
        except subprocess.TimeoutExpired as exc:
            raise TimeoutError(f"Command timed out after {effective_timeout} seconds") from exc


def execute_command(
    binary: str,
    args: Optional[List[str]] = None,
    timeout: float = 5.0,
    allowed_binaries: Optional[List[str]] = None,
) -> ExecutionResult:
    cmd_args = [binary] + (args if args is not None else [])
    runner = SafeCommandRunner(allowed_commands=allowed_binaries, default_timeout=timeout)
    return runner.run(cmd_args, timeout=timeout)
