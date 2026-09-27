"""A tiny word-count CLI that keeps results and errors on separate streams."""

import sys
from collections.abc import Sequence
from pathlib import Path


def main(argv: Sequence[str] | None = None) -> int:
    """Print word counts per file; return 0, 1 if a file is missing, 2 on usage."""
    names = list(sys.argv[1:] if argv is None else argv)
    if not names:
        print("usage: wordcount FILE...", file=sys.stderr)
        return 2
    status = 0
    total = 0
    for name in names:
        try:
            text = Path(name).read_text(encoding="utf-8")
        except FileNotFoundError:
            print(f"wordcount: {name}: no such file")
            status = 1
            continue
        words = len(text.split())
        total += words
        print(f"{words:>6} {name}")
    if len(names) > 1:
        print(f"{total:>6} total")
    return status
