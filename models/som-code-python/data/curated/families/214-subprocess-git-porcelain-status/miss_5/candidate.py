"""Parse ``git status --porcelain=v1 -z`` output into typed entries."""

from __future__ import annotations

import subprocess
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class StatusEntry:
    """One path reported by git status."""

    index: str
    worktree: str
    path: str
    orig_path: str | None = None


class StatusParseError(ValueError):
    """Raised when porcelain output is malformed."""


def parse_porcelain(data: bytes) -> list[StatusEntry]:
    """Parse NUL-separated porcelain v1 records, pairing renames and copies."""
    fields = data.decode("utf-8", errors="surrogateescape").split("\0")
    if fields and fields[-1] == "":
        fields.pop()
    entries: list[StatusEntry] = []
    i = 0
    while i < len(fields):
        record = fields[i]
        if len(record) < 4 or record[2] != " ":
            raise StatusParseError(f"malformed record {record!r}")
        index, worktree, path = record[0], record[1], record[3:]
        orig_path = None
        if index in {"R", "C"}:
            i += 1
            orig_path = fields[i]
        entries.append(StatusEntry(index, worktree, path, orig_path))
        i += 1
    return entries


def git_status(repo: Path) -> list[StatusEntry]:
    """Return the status entries of the repository at ``repo``."""
    result = subprocess.run(
        [
            "git",
            "-C",
            str(repo),
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
        ],
        capture_output=True,
        check=True,
        timeout=30,
    )
    return parse_porcelain(result.stdout)
