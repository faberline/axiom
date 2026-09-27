"""Stream files through hashlib and check them against a manifest of hex digests."""

import hashlib
from collections.abc import Mapping
from pathlib import Path

DEFAULT_CHUNK = 64 * 1024


def file_digest(
    path: Path, algorithm: str = "sha256", chunk_size: int = DEFAULT_CHUNK
) -> str:
    """Return the hex digest of a file read in fixed-size chunks."""
    if algorithm not in hashlib.algorithms_guaranteed:
        raise ValueError(f"unsupported algorithm: {algorithm}")
    if chunk_size < 1:
        raise ValueError("chunk_size must be positive")
    hasher = hashlib.new(algorithm)
    with path.open("rb") as handle:
        while chunk := handle.read(chunk_size):
            hasher.update(chunk)
    return hasher.hexdigest()


def verify_manifest(root: Path, manifest: Mapping[str, str]) -> list[str]:
    """Return the sorted names whose file is missing or whose digest differs."""
    bad: list[str] = []
    for name, expected in manifest.items():
        target = root / name
        if not target.is_file():
            continue
        if file_digest(target) != expected.lower():
            bad.append(name)
    return sorted(bad)
