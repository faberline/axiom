"""Reference assembler (L4): execute an L3 operation list against the snippet ISA.

Pure string assembly, no model and no execution. Each file is written as its
module docstring in triple double quotes, a blank line, its imports grouped
by ``group`` with one blank line between groups, then each block preceded by
its ``blank_before`` blank lines, and a single trailing newline
(``docs/reference/layer-records.md`` § Assembly). A future Rust assembler
must produce the same bytes.

Rules this module owns:

- The operation list is validated against its topology first
  (:func:`som_core.records.validate_ops`), so an assembler run never writes
  a partial program.
- A snippet renders by replacing each ``{{name}}`` in its template with
  ``params[name]``; an unknown snippet id, a parameter the template needs
  but ``params`` lacks, and a parameter the template never uses are all
  refused. A snippet's ``imports`` are not added: the operation list carries
  every import as ``ADD_IMPORT``.
"""

from __future__ import annotations

import json
import re
from pathlib import Path
from typing import Any

from .records import RecordError, validate_ops

PLACEHOLDER = re.compile(r"\{\{([a-zA-Z0-9_]+)\}\}")
DEFAULT_ISA = Path(__file__).resolve().parents[3] / "som-code-python" / "data" / "snippets"


def load_isa(isa_dir: Path | str) -> dict[str, dict[str, Any]]:
    """Every snippet under ``isa_dir/<library>/*.json``, keyed by id."""
    isa: dict[str, dict[str, Any]] = {}
    for path in sorted(Path(isa_dir).glob("*/*.json")):
        snippet = json.loads(path.read_text(encoding="utf-8"))
        if snippet["id"] in isa:
            raise RecordError(f"snippet id {snippet['id']!r} is defined twice in {isa_dir}")
        isa[snippet["id"]] = snippet
    return isa


def render_snippet(template: str, params: dict[str, str]) -> str:
    """Substitute every placeholder; refuse a missing or unused parameter."""
    needed = set(PLACEHOLDER.findall(template))
    missing = sorted(needed - params.keys())
    extra = sorted(params.keys() - needed)
    if missing:
        raise RecordError(f"snippet parameters missing: {missing}")
    if extra:
        raise RecordError(f"snippet parameters not in the template: {extra}")
    return PLACEHOLDER.sub(lambda m: params[m.group(1)], template)


def assemble(
    ops: list[dict[str, Any]],
    topology: dict[str, Any],
    isa: dict[str, dict[str, Any]] | None = None,
) -> dict[str, str]:
    """Return ``{path: text}`` for every file the operation list creates."""
    validate_ops(ops, topology)
    docstrings: dict[str, str | None] = {}
    imports: dict[str, dict[int, list[str]]] = {}
    blocks: dict[str, list[tuple[int, str]]] = {}
    for op in ops:
        path = op["path"]
        if op["op"] == "CREATE_FILE":
            docstrings[path] = op["docstring"]
            imports[path] = {}
            blocks[path] = []
        elif op["op"] == "ADD_IMPORT":
            imports[path].setdefault(op["group"], []).append(op["stmt"])
        elif op["op"] == "INSERT_SNIPPET":
            if isa is None or op["id"] not in isa:
                raise RecordError(f"unknown snippet {op['id']!r} for block {op['block']!r}")
            try:
                source = render_snippet(isa[op["id"]]["template"], op["params"])
            except RecordError as exc:
                raise RecordError(f"block {op['block']!r} ({op['id']}): {exc}") from exc
            blocks[path].append((op["blank_before"], source))
        else:
            blocks[path].append((op["blank_before"], op["source"]))

    files: dict[str, str] = {}
    for path, docstring in docstrings.items():
        head: list[str] = []
        if docstring is not None:
            head.append(f'"""{docstring}"""')
        groups = [imports[path][g] for g in sorted(imports[path])]
        if groups:
            head.append("\n\n".join("\n".join(group) for group in groups))
        text = "\n\n".join(head) + "\n" if head else ""
        for blank_before, source in blocks[path]:
            text += "\n" * blank_before + source.rstrip("\n") + "\n"
        files[path] = text
    return files


def load_ops_document(path: Path | str) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    """Read ``{"topology", "ops"}`` from a JSON file, or from a ``family.json``'s ``decompiled``."""
    doc = json.loads(Path(path).read_text(encoding="utf-8"))
    if "decompiled" in doc:
        doc = doc["decompiled"]
    if "ops" not in doc or "topology" not in doc:
        raise RecordError(f"{path}: needs 'topology' and 'ops' (or a family.json 'decompiled' block with both)")
    return doc["ops"], doc["topology"]


def write_files(files: dict[str, str], out: Path | str) -> list[Path]:
    """Write each assembled file under ``out``; return the written paths."""
    written = []
    for rel, text in files.items():
        target = Path(out) / rel
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text, encoding="utf-8")
        written.append(target)
    return written
