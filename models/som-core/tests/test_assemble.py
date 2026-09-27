"""The reference assembler writes exactly the bytes docs/reference/layer-records.md specifies."""

from __future__ import annotations

import json
from pathlib import Path

import pytest
from typer.testing import CliRunner

from som_core.assemble import DEFAULT_ISA, assemble, load_isa, parse_template, render_snippet
from som_core.cli import app
from som_core.records import RecordError

TOPOLOGY = {
    "files": [
        {
            "path": "app.py",
            "blocks": [
                {"name": "Item", "kind": "class", "depends_on": []},
                {"name": "engine", "kind": "constant", "depends_on": []},
                {"name": "factory", "kind": "constant", "depends_on": ["engine"]},
            ],
        }
    ]
}
ISA = {"pydantic_base": {"id": "pydantic_base", "template": "class {{model_name}}(BaseModel):\n    model_config = ConfigDict(from_attributes=True)"}}
OPS = [
    {"op": "CREATE_FILE", "path": "app.py", "docstring": "Demo module."},
    {"op": "ADD_IMPORT", "path": "app.py", "stmt": "from collections.abc import Generator", "group": 0},
    {"op": "ADD_IMPORT", "path": "app.py", "stmt": "from pydantic import BaseModel, ConfigDict", "group": 1},
    {"op": "ADD_IMPORT", "path": "app.py", "stmt": "from sqlalchemy import create_engine", "group": 1},
    {"op": "INSERT_SNIPPET", "path": "app.py", "block": "Item", "id": "pydantic_base", "params": {"model_name": "Item"}, "blank_before": 2},
    {"op": "INSERT_BLOCK", "path": "app.py", "block": "engine", "source": "# one engine per process\nengine = create_engine('sqlite://')", "blank_before": 2},
    {"op": "INSERT_BLOCK", "path": "app.py", "block": "factory", "source": "factory = engine.connect", "blank_before": 0},
]
EXPECTED = '''"""Demo module."""

from collections.abc import Generator

from pydantic import BaseModel, ConfigDict
from sqlalchemy import create_engine


class Item(BaseModel):
    model_config = ConfigDict(from_attributes=True)


# one engine per process
engine = create_engine('sqlite://')
factory = engine.connect
'''


def test_an_operation_list_assembles_to_the_specified_bytes() -> None:
    assert assemble(OPS, TOPOLOGY, ISA) == {"app.py": EXPECTED}


def test_a_file_without_docstring_or_imports_starts_with_its_first_block() -> None:
    topo = {"files": [{"path": "m.py", "blocks": [{"name": "X", "kind": "constant", "depends_on": []}]}]}
    ops = [
        {"op": "CREATE_FILE", "path": "m.py", "docstring": None},
        {"op": "INSERT_BLOCK", "path": "m.py", "block": "X", "source": "X = 1\n", "blank_before": 0},
    ]
    assert assemble(ops, topo) == {"m.py": "X = 1\n"}


def test_an_unknown_snippet_is_refused() -> None:
    with pytest.raises(RecordError, match="unknown snippet 'pydantic_base'"):
        assemble(OPS, TOPOLOGY, {})


@pytest.mark.parametrize(
    ("params", "message"),
    [({}, r"missing: \['model_name'\]"), ({"model_name": "A", "base": "B"}, r"not in the template: \['base'\]")],
)
def test_a_missing_or_extra_parameter_is_refused(params: dict[str, str], message: str) -> None:
    with pytest.raises(RecordError, match=message):
        render_snippet(ISA["pydantic_base"]["template"], params)


MODEL = "class {{name}}(BaseModel):\n{{#fields}}    {{field}}: {{type}}\n{{/fields}}"


def test_a_section_renders_its_body_once_per_item_in_order() -> None:
    params = {"name": "Item", "fields": [{"field": "id", "type": "int"}, {"field": "title", "type": "str"}]}
    assert render_snippet(MODEL, params) == "class Item(BaseModel):\n    id: int\n    title: str\n"


def test_an_empty_section_renders_nothing() -> None:
    assert render_snippet(MODEL, {"name": "Item", "fields": []}) == "class Item(BaseModel):\n"


@pytest.mark.parametrize(
    ("fields", "message"),
    [
        ([{"field": "id"}], r"missing in fields\[0\]: \['type'\]"),
        ([{"field": "id", "type": "int", "default": "0"}], r"not in the template in fields\[0\]: \['default'\]"),
        ("id: int", r"section 'fields' must be a list"),
        (["id"], r"section 'fields'\[0\] must be an object"),
    ],
)
def test_a_malformed_section_item_is_refused(fields: object, message: str) -> None:
    with pytest.raises(RecordError, match=message):
        render_snippet(MODEL, {"name": "Item", "fields": fields})


@pytest.mark.parametrize(
    ("template", "message"),
    [
        ("{{#a}}{{#b}}x{{/b}}{{/a}}", "nested"),
        ("{{#a}}x", "never closed"),
        ("{{#a}}x{{/b}}", "no matching open"),
        ("{{name}}{{#a}}{{name}}{{/a}}", r"reuses outer keys \['name'\]"),
    ],
)
def test_a_malformed_section_template_is_refused(template: str, message: str) -> None:
    with pytest.raises(RecordError, match=message):
        render_snippet(template, {})


def test_an_invalid_operation_list_writes_nothing() -> None:
    with pytest.raises(RecordError, match="never inserted"):
        assemble(OPS[:-1], TOPOLOGY, ISA)


def test_the_default_isa_loads_and_every_template_parses() -> None:
    isa = load_isa(DEFAULT_ISA)
    assert isa
    for snippet in isa.values():
        parse_template(snippet["template"])


def test_som_assemble_reads_a_family_json_and_writes_the_files(tmp_path: Path) -> None:
    isa_dir = tmp_path / "isa" / "pydantic"
    isa_dir.mkdir(parents=True)
    (isa_dir / "pydantic_base.json").write_text(json.dumps(ISA["pydantic_base"]), encoding="utf-8")
    family = tmp_path / "family.json"
    family.write_text(json.dumps({"id": "demo", "decompiled": {"topology": TOPOLOGY, "ops": OPS}}), encoding="utf-8")
    result = CliRunner().invoke(app, ["assemble", str(family), "--out", str(tmp_path / "out"), "--isa", str(tmp_path / "isa")])
    assert result.exit_code == 0, result.output
    assert (tmp_path / "out" / "app.py").read_text(encoding="utf-8") == EXPECTED


def test_som_assemble_refuses_with_exit_2(tmp_path: Path) -> None:
    doc = tmp_path / "ops.json"
    doc.write_text(json.dumps({"topology": TOPOLOGY, "ops": OPS[:-1]}), encoding="utf-8")
    result = CliRunner().invoke(app, ["assemble", str(doc), "--out", str(tmp_path / "out")])
    assert result.exit_code == 2
    assert "refused:" in result.output
    assert not (tmp_path / "out").exists()
