"""Run an allowlisted argument vector through subprocess without a shell."""

import subprocess
from dataclasses import dataclass


@dataclass
class CommandResult:
    """The decoded stdout, stderr, and exit code of a finished command."""

    stdout: str
    stderr: str
    exit_code: int


ExecutionResult = CommandResult


class SafeCommandRunner:
    """Execute argument vectors, optionally restricted to allowed commands."""

    def __init__(
        self,
        allowed_commands: list[str] | None = None,
        default_timeout: float = 10.0,
    ) -> None:
        self.allowed_commands: set[str] | None = (
            set(allowed_commands) if allowed_commands is not None else None
        )
        self.default_timeout = default_timeout

    def run(self, args: list[str], timeout: float | None = None) -> CommandResult:
        """Return the output of args, raising when refused, failed, or timed out."""
        if (
            not isinstance(args, list)
            or not args
            or not all(isinstance(a, str) for a in args)
        ):
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
                shell=False,
                check=True,
                timeout=effective_timeout,
            )
            return CommandResult(
                stdout=proc.stdout, stderr=proc.stderr, exit_code=proc.returncode
            )
        except subprocess.CalledProcessError as exc:
            raise RuntimeError(
                f"Command failed with exit code {exc.returncode}: {exc.stderr}"
            ) from exc
        except subprocess.TimeoutExpired as exc:
            raise TimeoutError(
                f"Command timed out after {effective_timeout} seconds"
            ) from exc


def execute_command(
    binary: str,
    args: list[str] | None = None,
    timeout: float = 5.0,
    allowed_binaries: list[str] | None = None,
) -> ExecutionResult:
    """Run binary with args under an optional allowlist and return its result."""
    cmd_args = [binary] + (args if args is not None else [])
    runner = SafeCommandRunner(
        allowed_commands=allowed_binaries, default_timeout=timeout
    )
    return runner.run(cmd_args, timeout=timeout)
