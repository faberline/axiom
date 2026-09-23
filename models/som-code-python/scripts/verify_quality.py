#!/usr/bin/env python3
"""Verify that every candidate in a corpus layer is idiomatic, typed Python.

The corpus teaches a generator what the Python community would accept, so
the reference program of each family has to pass the tools that community
runs, with the configuration in this project's ``pyproject.toml``:

- The gold candidate: ``ruff check`` (the full selection), ``ruff format
  --check``, ``mypy`` in strict mode, and ``pylint``, all with zero
  messages.
- Each near miss: ``ruff format --check`` and ``ruff check --select
  NEAR_MISS_RULES``, the style rules only. A near miss must look exactly as
  well written as the gold so that style never tells the two apart, while
  the behavioral rules (bugbear, pylint, strict typing) are left out because
  a near miss's defect may be precisely what they flag.

mypy and pylint run one file per process: every candidate is named
``candidate.py``, so one process would treat them as the same module.

Exit 0 when every candidate passes; exit 1 with one line per defect.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

PROJECT = Path(__file__).resolve().parent.parent
CONFIG = PROJECT / "pyproject.toml"
CORPUS = PROJECT / "data" / "curated"
BIN = Path(sys.executable).parent
NEAR_MISS_RULES = "E,W,I,UP"
PYLINT_LINE = re.compile(r"^[^:\s][^:]*:\d+: \[")
WORKERS = max(1, (os.cpu_count() or 2) - 1)


def tool(name: str) -> str:
    """Resolve a tool next to the running interpreter, as ``uv sync`` installs it."""
    path = BIN / name
    if not path.exists():
        raise SystemExit(f"{name} not found next to {sys.executable}; run `uv sync --project {PROJECT}`")
    return str(path)


def run(argv: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
    return subprocess.run(argv, capture_output=True, text=True, cwd=PROJECT, check=False, **kwargs)  # type: ignore[call-overload]


def label(path: Path, families: Path) -> str:
    rel = path.relative_to(families)
    return f"{rel.parts[0]}/{rel.parts[1]}"


def ruff_check(files: list[Path], families: Path, select: str | None) -> list[str]:
    if not files:
        return []
    argv = [tool("ruff"), "check", "--config", str(CONFIG), "--output-format", "json", "--no-cache"]
    if select:
        argv += ["--select", select]
    out = run([*argv, *map(str, files)])
    if out.returncode not in (0, 1):
        return [f"ruff check crashed: {out.stderr.strip()}"]
    return [
        f"{label(Path(d['filename']), families)}: ruff {d['code']} line {d['location']['row']}: {d['message']}"
        for d in json.loads(out.stdout or "[]")
    ]


def ruff_format(files: list[Path], families: Path) -> list[str]:
    out = run([tool("ruff"), "format", "--config", str(CONFIG), "--check", "--no-cache", *map(str, files)])
    if out.returncode not in (0, 1):
        return [f"ruff format crashed: {out.stderr.strip()}"]
    prefix = "Would reformat: "
    return [
        f"{label(Path(line[len(prefix):]).resolve(), families)}: ruff format would reformat"
        for line in out.stdout.splitlines()
        if line.startswith(prefix)
    ]


def mypy_chunk(files: list[Path], families: Path, cache: Path) -> list[str]:
    defects: list[str] = []
    for path in files:
        out = run([tool("mypy"), "--config-file", str(CONFIG), "--cache-dir", str(cache), "--no-error-summary", str(path)])
        for line in out.stdout.splitlines():
            if ": error:" in line:
                defects.append(f"{label(path, families)}: mypy {line.split(':', 1)[1].strip()}")
    return defects


def pylint_one(path: Path, families: Path) -> list[str]:
    out = run([tool("pylint"), "--rcfile", str(CONFIG), "--score=n", "--output-format=parseable", str(path)])
    return [
        f"{label(path, families)}: pylint {line.split(':', 1)[1].strip()}"
        for line in out.stdout.splitlines()
        if PYLINT_LINE.match(line)
    ]


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--corpus", type=Path, default=CORPUS, help="corpus layer to read (default data/curated; data/user for your own families)")
    ap.add_argument("families", nargs="*", help="family directory prefixes to check (default: all)")
    args = ap.parse_args()
    families = args.corpus.resolve() / "families"
    dirs = sorted(d for d in families.iterdir() if (d / "family.json").is_file()) if families.is_dir() else []
    if args.families:
        dirs = [d for d in dirs if any(d.name.startswith(p) for p in args.families)]
    if not dirs:
        print(f"no families under {families}")
        return 1
    golds: list[Path] = []
    misses: list[Path] = []
    for fam in dirs:
        meta = json.loads((fam / "family.json").read_text(encoding="utf-8"))
        for cand in meta["candidates"]:
            (golds if cand["kind"] == "gold" else misses).append(fam / cand["module"])

    defects = ruff_check(golds, families, None)
    defects += ruff_check(misses, families, NEAR_MISS_RULES)
    defects += ruff_format(golds + misses, families)
    cache_root = Path(tempfile.gettempdir()) / "som-quality-mypy"
    chunks = [golds[i::WORKERS] for i in range(WORKERS)]
    with ThreadPoolExecutor(WORKERS) as pool:
        typed = pool.map(lambda ic: mypy_chunk(ic[1], families, cache_root / str(ic[0])), enumerate(chunks))
        linted = pool.map(lambda p: pylint_one(p, families), golds)
        for batch in [*typed, *linted]:
            defects += batch

    print(f"families: {len(dirs)}  gold: {len(golds)}  near misses: {len(misses)}")
    for line in sorted(defects):
        print(f"DEFECT {line}")
    print(f"[RESULT: {'SUCCESS' if not defects else 'FAILURE'}] {len(defects)} quality defects")
    return 0 if not defects else 1


if __name__ == "__main__":
    sys.exit(main())
