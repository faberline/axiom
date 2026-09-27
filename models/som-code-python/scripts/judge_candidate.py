#!/usr/bin/env python3
"""Judge one generated program the way the corpus judges its gold candidate.

``judge_candidate.py [--corpus DIR] FAMILY CANDIDATE_DIR`` runs the family's
fixture on ``CANDIDATE_DIR/candidate.py`` exactly as
``scripts/verify_harness.py`` runs a gold (same isolation plugin, same
sanitized environment, same timeout), then the gold quality gate of
``scripts/verify_quality.py`` (full ``ruff check``, ``ruff format --check``,
strict ``mypy``, ``pylint``) on the same file. It prints one JSON object:
``{"family", "fixture": <exit code or null when the family has no fixture>,
"fixture_tail", "quality": [<message>...]}``. ``som eval`` in
``models/som-core`` is the caller; the exit code is 0 whenever a verdict was
printed, and 2 when the family or the candidate file does not exist.
"""

from __future__ import annotations

import argparse
import json
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from verify_harness import DATA, default_repo_root, find_fixture_test, find_python_executable, run_candidate_test  # noqa: E402
from verify_quality import CONFIG, PYLINT_LINE, run, tool  # noqa: E402


def quality(path: Path) -> list[str]:
    out: list[str] = []
    check = run([tool("ruff"), "check", "--config", str(CONFIG), "--output-format", "json", "--no-cache", str(path)])
    out += [f"ruff {d['code']} line {d['location']['row']}: {d['message']}" for d in json.loads(check.stdout or "[]")]
    if run([tool("ruff"), "format", "--config", str(CONFIG), "--check", "--no-cache", str(path)]).returncode == 1:
        out.append("ruff format would reformat")
    with tempfile.TemporaryDirectory(prefix="som-judge-mypy") as cache:
        typed = run([tool("mypy"), "--config-file", str(CONFIG), "--cache-dir", cache, "--no-error-summary", str(path)])
    out += [f"mypy {line.split(':', 1)[1].strip()}" for line in typed.stdout.splitlines() if ": error:" in line]
    linted = run([tool("pylint"), "--rcfile", str(CONFIG), "--score=n", "--output-format=parseable", str(path)])
    out += [f"pylint {line.split(':', 1)[1].strip()}" for line in linted.stdout.splitlines() if PYLINT_LINE.match(line)]
    return out


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--corpus", type=Path, default=DATA / "curated", help="corpus layer the family lives in")
    ap.add_argument("family", help="family directory name")
    ap.add_argument("candidate_dir", type=Path, help="directory holding the generated candidate.py")
    args = ap.parse_args()
    corpus = args.corpus.resolve()
    candidate = args.candidate_dir.resolve() / "candidate.py"
    if not (corpus / "families" / args.family / "family.json").is_file() or not candidate.is_file():
        print(f"refused: no family {args.family} under {corpus} or no {candidate}", file=sys.stderr)
        return 2
    test_file = find_fixture_test(corpus / "fixtures", args.family)
    fixture, tail = None, ""
    if test_file is not None:
        root = default_repo_root()
        fixture, stdout, stderr = run_candidate_test(find_python_executable(root), root, candidate.parent, test_file)
        tail = ((stdout or stderr).strip().splitlines() or [""])[-1]
    print(json.dumps({"family": args.family, "fixture": fixture, "fixture_tail": tail, "quality": quality(candidate)}))
    return 0


if __name__ == "__main__":
    sys.exit(main())
