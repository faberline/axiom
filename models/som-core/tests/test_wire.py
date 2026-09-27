"""The L3 wire format round-trips every committed operation list and refuses a malformed one by line."""

from __future__ import annotations

import json
import re

import pytest

from som_core.dataset import load_corpora
from som_core.records import RecordError
from som_core.wire import dump_block, dump_ops, load_block, load_ops

ROWS = [r for r in load_corpora() if (r["metadata"].get("decompiled") or {}).get("ops")]


def test_every_committed_operation_list_round_trips() -> None:
    assert len(ROWS) == 303
    for row in ROWS:
        ops = row["metadata"]["decompiled"]["ops"]
        assert load_ops(dump_ops(ops)) == ops, row["id"]


def test_no_committed_source_carries_a_bare_fence_line() -> None:
    for row in ROWS:
        for op in row["metadata"]["decompiled"]["ops"]:
            assert "```" not in op.get("source", "").split("\n"), row["id"]


@pytest.mark.parametrize(
    ("text", "message"),
    [
        ('{"op":"INSERT_BLOCK","path":"m.py","block":"f"}\n```python\ndef f(): ...', "line 2: ```python fence never closes"),
        ('{"op":"CREATE_FILE","path":"m.py"}\nimport os', "line 2: not a JSON operation"),
        ('{"op":"INSERT_BLOCK","path":"m.py","block":"f"}\n{"op":"CREATE_FILE","path":"m.py"}', "line 1: INSERT_BLOCK is not followed"),
        ('{"op":"INSERT_BLOCK","path":"m.py","block":"f","source":"x"}', "line 1: INSERT_BLOCK carries source"),
        ("[1]", "line 1: operation is not a JSON object"),
    ],
)
def test_a_malformed_text_is_refused_with_its_line(text: str, message: str) -> None:
    with pytest.raises(RecordError, match=re.escape(message)):
        load_ops(text)


def test_blank_lines_between_operations_are_ignored() -> None:
    assert load_ops('\n{"op":"CREATE_FILE","path":"m.py"}\n\n') == [{"op": "CREATE_FILE", "path": "m.py"}]


def test_every_committed_block_operation_round_trips_as_a_block_reply() -> None:
    for row in ROWS:
        for op in row["metadata"]["decompiled"]["ops"]:
            if op["op"] in ("INSERT_BLOCK", "INSERT_SNIPPET"):
                reply = dump_block(op)
                assert not {"path", "block", "source"} & json.loads(reply.split("\n")[0]).keys()
                assert load_block(reply, op["path"], op["block"]) == op, row["id"]


@pytest.mark.parametrize(
    ("text", "message"),
    [
        ('{"op":"INSERT_BLOCK","path":"m.py","block":"f","blank_before":2}\n```python\nx = 1', "the header carries ['path', 'block']"),
        ('{"op":"INSERT_BLOCK","blank_before":2}\ndef f(): ...', "line 2: INSERT_BLOCK is not followed"),
        ('{"op":"ADD_IMPORT","stmt":"import os","group":0}', "expected one of ['INSERT_BLOCK', 'INSERT_SNIPPET']"),
        ('{"op":"INSERT_SNIPPET","id":"x","params":{},"blank_before":2}\nextra', "line 2: an INSERT_SNIPPET reply ends"),
        ("class F:", "line 1: not a JSON operation"),
    ],
)
def test_a_malformed_block_reply_is_refused(text: str, message: str) -> None:
    with pytest.raises(RecordError, match=re.escape(message)):
        load_block(text, "m.py", "f")
