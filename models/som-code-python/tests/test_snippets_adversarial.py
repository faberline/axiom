"""scripts/verify_snippets.py refuses every snippet ISA its docstring forbids.

Each case builds a two-snippet ISA and a three-family corpus in a temporary
directory, applies one mutation, and runs the real script on them through
``--snippets`` and ``--corpus``. The last case runs it on the committed ISA
and corpus, which must pass and stay byte-identical.
"""

from __future__ import annotations

import hashlib
import json
import subprocess
import sys
from collections.abc import Callable
from pathlib import Path

import pytest

PROJECT = Path(__file__).resolve().parent.parent
SCRIPT = PROJECT / "scripts" / "verify_snippets.py"

GREET = {
    "id": "greet",
    "description": "Print a greeting.",
    "imports": [],
    "template": 'print("hello {{name}}")',
}
RECORD = {
    "id": "record",
    "description": "A dataclass with one line per field.",
    "imports": ["from dataclasses import dataclass"],
    "template": "@dataclass\nclass {{name}}:\n{{#fields}}\n    {{field}}: {{type}}{{/fields}}",
}
FIELDS = [{"field": "id", "type": "int"}]


def _snippet_op(snippet_id: str, params: dict[str, object]) -> dict[str, object]:
    return {"op": "INSERT_SNIPPET", "id": snippet_id, "params": params}


def _corpus() -> dict[str, list[dict[str, object]]]:
    return {
        "a": [_snippet_op("greet", {"name": "a"}), _snippet_op("record", {"name": "A", "fields": FIELDS})],
        "b": [_snippet_op("greet", {"name": "b"}), _snippet_op("record", {"name": "B", "fields": []})],
        "c": [{"op": "INSERT_BLOCK", "source": "x = 1"}],
    }


Mutation = Callable[[dict[str, dict[str, object]], dict[str, list[dict[str, object]]]], None]


def _run(tmp_path: Path, mutate: Mutation | None = None, raw: dict[str, str] | None = None) -> tuple[int, str]:
    isa: dict[str, dict[str, object]] = {"lib/greet.json": dict(GREET), "lib/record.json": dict(RECORD)}
    corpus = _corpus()
    if mutate is not None:
        mutate(isa, corpus)
    files = {path: json.dumps(snippet) for path, snippet in isa.items()} | (raw or {})
    for path, text in files.items():
        target = tmp_path / "snippets" / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text, encoding="utf-8")
    for family, ops in corpus.items():
        target = tmp_path / "corpus" / "families" / family / "family.json"
        target.parent.mkdir(parents=True)
        target.write_text(json.dumps({"decompiled": {"ops": ops}}), encoding="utf-8")
    res = subprocess.run(
        [sys.executable, str(SCRIPT), "--snippets", str(tmp_path / "snippets"), "--corpus", str(tmp_path / "corpus")],
        capture_output=True,
        text=True,
        check=False,
    )
    return res.returncode, res.stdout + res.stderr


def test_a_clean_isa_passes_and_reports_coverage(tmp_path: Path) -> None:
    rc, out = _run(tmp_path)
    assert rc == 0, out
    assert "greet                              2 families    2 blocks" in out
    assert "coverage: 4 / 5 blocks are INSERT_SNIPPET" in out
    assert "[RESULT: SUCCESS] 0 snippet defects" in out


@pytest.mark.parametrize(
    ("mutate", "defect"),
    [
        (lambda isa, _: isa["lib/greet.json"].update(id="hello"), "lib/greet.json: id 'hello' does not match the file name 'greet'"),
        (lambda isa, _: isa["lib/greet.json"].pop("description"), "lib/greet.json: fields must be"),
        (lambda isa, _: isa["lib/greet.json"].update(extra=1), "lib/greet.json: fields must be"),
        (lambda isa, _: isa["lib/greet.json"].update(template="  "), "lib/greet.json: template must be a non-empty string"),
        (lambda isa, _: isa["lib/greet.json"].update(imports="import os"), "lib/greet.json: imports must be a list"),
        (lambda isa, _: isa["lib/record.json"].update(imports=["print(1)"]), "lib/record.json: import 'print(1)' is not one import statement"),
        (
            lambda isa, _: isa["lib/record.json"].update(template=RECORD["template"] + "\n\n    {{body}}"),
            "lib/record.json: template line 6 '    {{body}}' is only a placeholder",
        ),
        (
            lambda isa, _: isa["lib/record.json"].update(template="class {{name}}:\n{{#fields}}\n    {{field}}{{#x}}: {{type}}{{/x}}{{/fields}}"),
            "lib/record.json: section 'x' is nested",
        ),
        (lambda _, corpus: corpus["b"].pop(0), "lib/greet.json: used by 1 families ['a']; a snippet needs at least 2"),
        (lambda _, corpus: corpus["c"].append(_snippet_op("ghost", {})), "ops use snippet 'ghost', which the ISA does not define"),
        (
            lambda isa, _: isa["lib/greet.json"].update(template="print({{name}} hello)"),
            "lib/greet.json: first use in a does not render to Python",
        ),
    ],
    ids=[
        "id-not-file-name",
        "missing-key",
        "extra-key",
        "blank-template",
        "imports-not-list",
        "import-not-import",
        "whole-line-hole",
        "nested-section",
        "one-family",
        "unknown-id",
        "not-python",
    ],
)
def test_a_mutated_isa_is_refused_by_name(tmp_path: Path, mutate: Mutation, defect: str) -> None:
    rc, out = _run(tmp_path, mutate)
    assert rc == 1, out
    assert any(line.startswith("DEFECT ") and defect in line for line in out.splitlines()), out
    assert "[RESULT: FAILURE]" in out


def test_invalid_json_is_refused_by_name(tmp_path: Path) -> None:
    rc, out = _run(tmp_path, raw={"lib/broken.json": '{"id": "broken",'})
    assert rc == 1, out
    assert "DEFECT lib/broken.json: not valid JSON" in out


def _digest(root: Path) -> str:
    h = hashlib.sha256()
    for path in sorted(root.rglob("*.json")):
        h.update(str(path.relative_to(root)).encode() + b"\0" + path.read_bytes())
    return h.hexdigest()


def test_the_committed_isa_passes_and_is_left_untouched() -> None:
    before = {name: _digest(PROJECT / "data" / name) for name in ("snippets", "curated")}
    res = subprocess.run([sys.executable, str(SCRIPT)], capture_output=True, text=True, check=False)
    assert res.returncode == 0, res.stdout + res.stderr
    assert "[RESULT: SUCCESS] 0 snippet defects" in res.stdout
    assert {name: _digest(PROJECT / "data" / name) for name in ("snippets", "curated")} == before
