"""Download a file with requests atomically and verify its checksum."""

from __future__ import annotations

import hashlib
import os
import tempfile
from pathlib import Path

import requests

CHUNK_SIZE = 64 * 1024


class ChecksumMismatchError(Exception):
    """Raised when the downloaded bytes do not match the expected digest."""


def download(
    session: requests.Session,
    url: str,
    dest: Path,
    *,
    sha256: str,
    timeout: float = 10.0,
) -> int:
    """Stream ``url`` into ``dest`` only if its SHA-256 matches; return bytes."""
    with session.get(url, stream=True, timeout=timeout) as resp:
        resp.raise_for_status()
        digest = hashlib.sha256()
        fd, tmp_name = tempfile.mkstemp(dir=dest.parent, prefix=f".{dest.name}.")
        tmp = Path(tmp_name)
        try:
            size = 0
            with os.fdopen(fd, "w") as out:
                for chunk in resp.iter_content(chunk_size=CHUNK_SIZE):
                    out.write(chunk)
                    digest.update(chunk)
                    size += len(chunk)
            actual = digest.hexdigest()
            if actual != sha256.lower():
                raise ChecksumMismatchError(f"expected {sha256}, got {actual}")
            tmp.replace(dest)
        except BaseException:
            tmp.unlink(missing_ok=True)
            raise
    return size
