"""The L3 wire format: the text an L3 model reads and writes for an operation list.

Rules this module owns:

- The record is the operation list ``docs/reference/layer-records.md``
  defines; this format only serializes it. :func:`load_ops` of
  :func:`dump_ops` returns the list unchanged, and ``records.py`` and the
  assembler never see the text.
- Each operation is one line of compact JSON. An ``INSERT_BLOCK`` line
  carries every field but ``source``; its source follows verbatim, unescaped,
  between a ```` ```python ```` line and a line that is exactly ```` ``` ````,
  because code escaped into a JSON string is where generated L3 records broke
  (unterminated strings, unbalanced brackets).
- Blank lines between operations are ignored. A line that is not a JSON
  object, an ``INSERT_BLOCK`` without its fence, a fence that never closes,
  and an ``INSERT_BLOCK`` line that also carries ``source`` are refused with
  the line number.
- A block-wise L3 reply (:func:`dump_block`) is one block operation whose
  header omits ``path`` and ``block`` (the prompt's ``NEXT`` line fixes
  them, and copying them is where replies misspelled the block) and whose
  ``INSERT_BLOCK`` fence is closed by the end of the reply, not by a
  ```` ``` ```` line the model must remember to write.
"""

from __future__ import annotations

import json
from typing import Any

from .records import RecordError

FENCE_OPEN = "```python"
FENCE_CLOSE = "```"


def dump_ops(ops: list[dict[str, Any]]) -> str:
    out = []
    for op in ops:
        if op.get("op") == "INSERT_BLOCK":
            header = {k: v for k, v in op.items() if k != "source"}
            out.append(json.dumps(header, ensure_ascii=False, separators=(",", ":")))
            out.append(f"{FENCE_OPEN}\n{op['source']}\n{FENCE_CLOSE}")
        else:
            out.append(json.dumps(op, ensure_ascii=False, separators=(",", ":")))
    return "\n".join(out)


def load_ops(text: str) -> list[dict[str, Any]]:
    lines = text.split("\n")
    ops: list[dict[str, Any]] = []
    i = 0
    while i < len(lines):
        line = lines[i]
        i += 1
        if not line.strip():
            continue
        try:
            op = json.loads(line)
        except json.JSONDecodeError as exc:
            raise RecordError(f"line {i}: not a JSON operation ({exc.msg})") from exc
        if not isinstance(op, dict):
            raise RecordError(f"line {i}: operation is not a JSON object")
        if op.get("op") == "INSERT_BLOCK":
            if "source" in op:
                raise RecordError(f"line {i}: INSERT_BLOCK carries source in its header; it belongs in the fence")
            if i >= len(lines) or lines[i] != FENCE_OPEN:
                raise RecordError(f"line {i}: INSERT_BLOCK is not followed by a {FENCE_OPEN} line")
            start = i + 1
            try:
                end = lines.index(FENCE_CLOSE, start)
            except ValueError as exc:
                raise RecordError(f"line {i + 1}: {FENCE_OPEN} fence never closes") from exc
            op["source"] = "\n".join(lines[start:end])
            i = end + 1
        ops.append(op)
    return ops


BLOCK_OPS = ("INSERT_BLOCK", "INSERT_SNIPPET")
BLOCK_KEYS = ("path", "block", "source")


def dump_block(op: dict[str, Any]) -> str:
    """One block operation as a block-wise L3 reply: the JSON header without the
    ``path`` and ``block`` the prompt's ``NEXT`` line already fixes, and for an
    ``INSERT_BLOCK`` a ```` ```python ```` line and the source running to the end
    of the reply, which closes it."""
    header = json.dumps({k: v for k, v in op.items() if k not in BLOCK_KEYS}, ensure_ascii=False, separators=(",", ":"))
    return f"{header}\n{FENCE_OPEN}\n{op['source']}" if op.get("op") == "INSERT_BLOCK" else header


def load_block(text: str, path: str, block: str) -> dict[str, Any]:
    """The inverse of :func:`dump_block` for the ``NEXT`` block ``path:block``."""
    head, _, rest = text.lstrip("\n").partition("\n")
    try:
        op = json.loads(head)
    except json.JSONDecodeError as exc:
        raise RecordError(f"line 1: not a JSON operation ({exc.msg})") from exc
    if not isinstance(op, dict) or op.get("op") not in BLOCK_OPS:
        raise RecordError(f"line 1: expected one of {list(BLOCK_OPS)}, got {op.get('op') if isinstance(op, dict) else op!r}")
    if carried := [k for k in BLOCK_KEYS if k in op]:
        raise RecordError(f"line 1: the header carries {carried}; NEXT fixes path and block, the fence holds source")
    if op["op"] == "INSERT_SNIPPET":
        if rest.strip():
            raise RecordError("line 2: an INSERT_SNIPPET reply ends after its header")
        return {"op": op.pop("op"), "path": path, "block": block, **op}
    fence, _, source = rest.partition("\n")
    if fence != FENCE_OPEN:
        raise RecordError(f"line 2: INSERT_BLOCK is not followed by a {FENCE_OPEN} line")
    return {"op": op.pop("op"), "path": path, "block": block, "source": source, **op}
