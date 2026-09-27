"""The SOM prompt language: render/parse round-trip, refusals, and the fixture contract."""

from __future__ import annotations

import pytest

from som_core.prompt import PromptError, contract_fields, expand_contract, parse_prompt, render_prompt, revise

FIELDS = {"intent": "POST /items returns 201", "exports": ["app", "reset_db"], "raises": ["ValueError 'bad'"], "avoid": ["ge=0 on price"]}


def test_render_then_parse_is_the_identity_in_tag_order() -> None:
    text = render_prompt({"avoid": FIELDS["avoid"], **FIELDS, "keys": []})
    assert text.splitlines()[0].startswith("intent:") and text.splitlines()[-1].startswith("avoid:")
    assert "keys" not in text
    assert parse_prompt(text) == FIELDS


@pytest.mark.parametrize(
    ("text", "message"),
    [
        ("exports: app", "missing required tag intent"),
        ("intent: x\nnote: y", "line 2"),
        ("intent: x\nintent: y", "duplicate tag intent"),
        ("intent: x\njust prose", "line 2"),
    ],
)
def test_parse_refuses_and_names_the_line(text: str, message: str) -> None:
    with pytest.raises(PromptError, match=message):
        parse_prompt(text)


FIXTURE = '''
import pytest
import candidate
from candidate import app

def test_x(monkeypatch):
    seen = []
    def fake(**kw):
        seen.append(kw.get("timeout"))
    monkeypatch.setattr(candidate, "fetch", fake)
    r = app.get("/x", headers={"X-Key": "k"})
    assert seen[0] == 5
    assert r.json()["detail"] == "Not found"
    with pytest.raises(ValueError, match="bad id"):
        candidate.parse("")
'''


def test_contract_fields_reads_names_and_values_not_test_bodies() -> None:
    got = contract_fields(FIXTURE)
    assert got["exports"] == ["app", "parse"]
    assert got["patch"] == ["candidate.fetch"]
    assert got["kwargs"] == ["timeout"] and got["asserts"] == ["timeout == 5"]
    assert got["raises"] == ["ValueError 'bad id'"]
    assert got["keywords"] == ["headers"]
    assert {"X-Key", "detail"} <= set(got["keys"]) and "'Not found'" in got["values"]


def test_calls_carries_one_shape_per_export_and_method():
    src = (
        "from candidate import Cipher\n"
        "def test_a():\n    c = Cipher(b'k')\n    ct, nonce = c.encrypt(b'm', associated_data=b'a')\n"
        "def test_b():\n    c = Cipher(b'j')\n    c.encrypt(b'x')\n"
    )
    assert contract_fields(src)["calls"] == ["c = Cipher(b'k')", "ct, nonce = c.encrypt(b'm', associated_data=b'a')"]


def test_a_fixture_returning_an_export_binds_its_parameter():
    src = (
        "import pytest\nfrom candidate import Parser\n"
        "@pytest.fixture\ndef parser():\n    return Parser()\n"
        "def test_a(parser):\n    data = parser.parse(text)\n"
    )
    fields = contract_fields(src)
    assert fields["calls"] == ["data = parser.parse(text)"]
    assert fields["attrs"] == ["parse"]


def test_fix_lines_join_the_last_intent_and_a_full_prompt_replaces_it() -> None:
    last = "intent: parse a date\nexports: parse"
    assert revise(last, "fix: reject empty\nfix: strip first") == "intent: parse a date; reject empty; strip first\nexports: parse"
    assert revise(last, "intent: other") == "intent: other"
    with pytest.raises(PromptError, match="previous prompt"):
        revise(None, "fix: x")


def test_a_contract_line_fills_the_omitted_tags_from_its_file_and_the_prompt_wins(tmp_path) -> None:
    (tmp_path / "contract.som").write_text("exports: Cipher\ncalls: c = Cipher(k)\nraises: ValueError 'short key'\n")
    got = parse_prompt(expand_contract("intent: seal bytes\ncalls: c = Cipher(key)\ncontract: contract.som", tmp_path))
    assert got == {"intent": "seal bytes", "exports": ["Cipher"], "calls": ["c = Cipher(key)"], "raises": ["ValueError 'short key'"]}
    assert expand_contract("fix: pad first", tmp_path) == "fix: pad first"
    with pytest.raises(PromptError, match="no file"):
        expand_contract("intent: x\ncontract: ../outside.som", tmp_path)
