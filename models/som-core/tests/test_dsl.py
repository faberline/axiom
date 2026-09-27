"""compile_dsl's rules and the lossless gold -> to_dsl -> compile_dsl round trip."""

from __future__ import annotations

import ast
from pathlib import Path

import pytest

from som_core.dataset import load_corpora
from som_core.dsl import _import_lines, compile_dsl, drop_docstrings, near_miss, to_dsl
from som_core.dsl_pairs import gold_text
from som_core.prompt import parse_prompt


def _rules(code: str) -> list[str]:
    return near_miss(ast.parse(code))


@pytest.mark.parametrize(
    "code",
    [
        "def f(a, b=[]):\n    return a\n",
        "try:\n    x = 1\nexcept:\n    pass\n",
        "def ok(token, given):\n    return token == given\n",
        "def peek(t):\n    return jwt.decode(t, key)\n",
        "def run(c):\n    return subprocess.run(c, shell=True)\n",
        "def load(s):\n    return yaml.load(s)\n",
        "def clean(v):\n    return re.sub(r\"[ -()]\", \"\", v)\n",
        "def save(p, s):\n    with tempfile.NamedTemporaryFile(\"w\", dir=p.parent, delete=False) as t:\n"
        "        t.write(s)\n    os.replace(t.name, p)\n",
    ],
)
def test_near_miss_rules_reject_the_curated_failure_modes(code: str) -> None:
    assert _rules(code)
    _, diags = compile_dsl(code)
    assert diags


def test_unverified_jwt_peek_and_digest_equality_pass() -> None:
    assert not _rules("def iss(t):\n    return jwt.decode(t, options={'verify_signature': False})['iss']\n")
    assert not _rules("def same(a, b):\n    return a.digest == b.digest\n")


def test_compile_adds_imports_docstring_exports_and_forward_future() -> None:
    code = "class Node(BaseModel):\n    child: Leaf | None = None\n\nclass Leaf(BaseModel):\n    at: datetime\n\ndef now() -> datetime:\n    return datetime.now()\n"
    src, diags = compile_dsl(code, "A node.", ["Node", "Leaf", "now"])
    assert diags == []
    tree = ast.parse(src)
    assert ast.get_docstring(tree) == "A node."
    imports = {st for n in tree.body if isinstance(n, (ast.Import, ast.ImportFrom)) for _, st in _import_lines(n)}
    assert {"from __future__ import annotations", "from pydantic import BaseModel", "from datetime import datetime"} <= imports
    assert "__all__" not in src  # only a re-exported import needs one


def test_compile_adds_future_only_for_an_eager_annotation_with_no_runtime_subscript() -> None:
    def future(code: str) -> bool:
        src, diags = compile_dsl(code)
        assert diags == []
        return "from __future__ import annotations" in src

    assert future("def f(x: pd.Series) -> pd.Series[float]:\n    return x\n")  # TypeError at def time without it
    assert future("class Row:\n    s: pd.Series[int]\n")
    assert not future("def f(q: asyncio.Queue[int]) -> None:\n    q.put_nowait(1)\n")  # Queue has __class_getitem__
    assert not future("def f() -> None:\n    x: pd.Series[int] = pd.Series([1])\n    print(x)\n")  # never evaluated


def test_compile_rejects_an_unknown_name_and_a_missing_export() -> None:
    _, diags = compile_dsl("def f():\n    return frobnicate_xyz()\n")
    assert any("frobnicate_xyz" in d for d in diags)
    _, diags = compile_dsl("def f():\n    return 1\n", "", ["zq_export"])
    assert diags == ["exports lists zq_export but the module never defines it; keep every earlier definition and add zq_export"]


def _norm(src: str) -> tuple[str, list[str]]:
    tree = ast.parse(src)
    body = [n for n in tree.body if not isinstance(n, (ast.Import, ast.ImportFrom))]
    if body and isinstance(body[0], ast.Expr) and isinstance(body[0].value, ast.Constant):
        body = body[1:]
    body = [n for n in body if not (isinstance(n, ast.Assign) and any(getattr(t, "id", "") == "__all__" for t in n.targets))]
    imports = sorted(st for n in tree.body if isinstance(n, (ast.Import, ast.ImportFrom)) for _, st in _import_lines(n))
    return ast.dump(ast.Module(body=body, type_ignores=[])), imports


def test_every_gold_round_trips_through_to_dsl_and_compile_dsl() -> None:
    bad = []
    for row in load_corpora():
        gold = gold_text(row)
        code, doc = to_dsl(gold)
        exports = [n.value for a in ast.parse(gold).body if isinstance(a, ast.Assign) and any(getattr(t, "id", "") == "__all__" for t in a.targets)
                   for n in getattr(a.value, "elts", []) if isinstance(n, ast.Constant)]
        src, diags = compile_dsl(code, doc, exports)
        if diags or _norm(src) != _norm(gold):
            bad.append((row["family"], diags[:2]))
    assert bad == []


def test_a_name_no_training_family_imports_resolves_through_the_symbol_index() -> None:
    src, diags = compile_dsl("def seal(k, d):\n    try:\n        return AESGCM(k).encrypt(b'n', d, None)\n    except InvalidTag:\n        raise\n")
    assert diags == []
    assert "from cryptography.hazmat.primitives.ciphers.aead import AESGCM" in src
    assert "from cryptography.exceptions import InvalidTag" in src


def test_an_indexed_package_named_bare_is_imported_whole() -> None:
    src, diags = compile_dsl("def f(c):\n    try:\n        c.x()\n    except redis.WatchError:\n        return 1\n")
    assert diags == []
    assert "import redis\n" in src


def test_symbol_table_errors_are_rejected():
    source, diags = compile_dsl("def f(a, a):\n    return a\n", exports=["f"])
    assert source == ""
    assert diags == ["L1: syntax: duplicate argument 'a' in function definition"]


def test_a_flagship_prompt_gains_the_contract_tags_it_lacks_but_keeps_its_own(tmp_path: Path) -> None:
    from som_core.dsl_pairs import flagship_prompts

    (tmp_path / "fixtures").mkdir()
    (tmp_path / "fixtures" / "test_f_x.py").write_text(
        "from candidate import Cipher\ndef test_a():\n    c = Cipher(b'k')\n    c.seal(b'm', aad=b'a')\n"
    )
    (tmp_path / "p").mkdir()
    (tmp_path / "p" / "f-x.json").write_text('{"prompts": ["intent: seal\\nexports: Cipher", "intent: s\\ncalls: c = Cipher(k)"]}')
    got = [parse_prompt(p) for p in flagship_prompts({"family": "f-x"}, tmp_path / "p", tmp_path)]
    assert got[0]["calls"] == ["c = Cipher(b'k')", "c.seal(b'm', aad=b'a')"] and got[0]["exports"] == ["Cipher"]
    assert got[1]["calls"] == ["c = Cipher(k)"]


def test_an_attribute_the_imported_package_lacks_is_rejected_and_a_lazy_submodule_is_not():
    _, diags = compile_dsl("import yaml\nx = yaml.NonScalarNode\ny = yaml.nodes.ScalarNode\n")
    assert diags == ["no such attribute in the installed package: yaml.NonScalarNode"]
    assert compile_dsl("import xml\nq = xml.etree.ElementTree.parse\n")[1] == []
    assert compile_dsl("from sqlalchemy import Base\nclass A(Base):\n    pass\n")[1] == ["no such attribute in the installed package: sqlalchemy.Base"]
    code = "from cryptography.hazmat.primitives.ciphers import algorithms\nfrom yaml import SafeLoader\nk = algorithms.AESGCM\nc = SafeLoader.x\n"
    assert compile_dsl(code)[1] == ["no such attribute in the installed package: cryptography.hazmat.primitives.ciphers.algorithms.AESGCM"]


def test_a_name_used_above_its_own_definition_is_hoisted_not_imported_from_a_package():
    code = 'from sqlalchemy import create_engine\nprint(engine)\nengine = create_engine("sqlite://")\n'
    source, diags = compile_dsl(code)
    assert diags == [] and "from sqlalchemy import engine" not in source
    assert source.index("engine = create_engine") < source.index("print(engine)")
    assert compile_dsl("def f():\n    return 1\n\n\nprint(x)\nx = f()\ny = x\n")[1] == []
    assert compile_dsl("class A(B):\n    pass\n\n\nclass B:\n    pass\n")[1] == [
        "used above its module-level definition (move the definition up): B"
    ]
    assert compile_dsl("class A:\n    def f(self) -> B:\n        return B()\n\n\nclass B:\n    pass\n")[1] == []


def test_fix_pairs_turn_each_distinct_near_miss_into_a_revision_to_the_gold():
    import json

    from som_core.dsl_pairs import fix_pairs

    corpus = Path(__file__).resolve().parents[2] / "som-code-python" / "data" / "curated"
    rows = [r for r in load_corpora([corpus]) if any(c.get("why_wrong") for c in r["candidates"])]
    row = rows[0]
    pairs = fix_pairs(row, corpus)
    gold, _ = to_dsl(gold_text(row))
    assert pairs and all(layer == "dsl_fix" and target == gold for layer, _, target in pairs)
    prompt, miss, feedback = json.loads(pairs[0][1])
    assert miss != gold and feedback[0].startswith("fix: ")


def test_retry_pairs_carry_the_loops_own_rejection_and_the_gold_clears_it():
    import json

    from som_core.dsl_pairs import mechanical_prompt, retry_pairs
    from som_core.generate import rejection
    from som_core.prompt import parse_prompt

    corpus = Path(__file__).resolve().parents[2] / "som-code-python" / "data" / "curated"
    row = next(r for r in load_corpora([corpus]) if r["family"] == "105-stdlib-itertools-batch-and-window")
    gold, _ = to_dsl(gold_text(row))
    fields = parse_prompt(mechanical_prompt(row, corpus))
    pairs = retry_pairs(row, corpus)
    assert pairs and all(layer == "dsl_retry" and target == gold for layer, _, target in pairs)
    assert rejection(fields, gold)[1:] == ([], [])
    feedback = [json.loads(payload)[2] for _, payload, _ in pairs]
    assert ["raises: no ValueError message contains 'size must be at least 1'"] in feedback
    unguarded = gold.replace('    if width < 1:\n        raise ValueError("width must be at least 1")\n', "")
    assert unguarded != gold and unguarded in [json.loads(payload)[1] for _, payload, _ in pairs]
    for _, payload, _ in pairs:
        _, attempt, said = json.loads(payload)
        _, diags, misses = rejection(fields, attempt)
        assert attempt != gold and said == diags + misses


def test_a_second_dropout_copy_keeps_the_first_draw_and_adds_its_own():
    from som_core.dsl_pairs import dsl_pairs

    corpus = Path(__file__).resolve().parents[2] / "som-code-python" / "data" / "curated"
    row = next(r for r in load_corpora([corpus]) if r["family"] == "105-stdlib-itertools-batch-and-window")
    none, one, two = (dsl_pairs(row, corpus, dropout=d, dropout_copies=n) for d, n in ((False, 1), (True, 1), (True, 2)))
    assert [p for p in one if p in none] == none and len(one) > len(none)
    assert two[: len(one)] == one and len(two) > len(one)


def test_fix_retry_pairs_name_the_cut_call_in_the_loops_own_words_and_the_gold_clears_it():
    import json

    from som_core.dsl_pairs import _fix_knockouts, fix_retry_pairs
    from som_core.generate import rejection

    corpus = Path(__file__).resolve().parents[2] / "som-code-python" / "data" / "curated"
    row = next(r for r in load_corpora([corpus]) if r["family"] == "105-stdlib-itertools-batch-and-window")
    gold, _ = to_dsl(gold_text(row))
    pairs = fix_retry_pairs(row, corpus)
    assert pairs and all(layer == "dsl_fix_retry" and target == gold for layer, _, target in pairs)
    for _, payload, _ in pairs:
        prompt, attempt, said = json.loads(payload)
        fields = parse_prompt(prompt)
        line = fields["intent"].rsplit("; ", 1)[1]
        _, diags, misses = rejection(fields, attempt, [line])
        assert said == diags + misses and any(m.startswith(f"fix: {line!r} but the code never ") for m in said)
        assert not any(m.startswith("fix:") for m in rejection(fields, gold, [line])[2])
    cuts = {repr(names): cut for _, names, cut in _fix_knockouts('def f(x):\n    g(x, mode="before", n=3)\n    h()\n')}
    assert cuts["{'c': 'g', 'k': 'mode', 'v': '\"before\"'}"] == 'def f(x):\n    g(x, n=3)\n    h()\n'
    assert cuts["{'c': 'h'}"] == 'def f(x):\n    g(x, mode="before", n=3)\n'
    assert cuts["{'c': 'g'}"] == "def f(x):\n    h()\n"
    assert _fix_knockouts("def f():\n    h()\n")[0][2] == "def f():\n    pass\n"


def test_swap_pairs_change_a_raises_message_alike_in_prompt_and_gold():
    from som_core.dsl_pairs import mechanical_prompt, swap_pairs
    from som_core.generate import rejection

    corpus = Path(__file__).resolve().parents[2] / "som-code-python" / "data" / "curated"
    row = next(r for r in load_corpora([corpus]) if r["family"] == "105-stdlib-itertools-batch-and-window")
    gold, _ = to_dsl(gold_text(row))
    pairs = swap_pairs(row, corpus)
    assert pairs and all(layer == "dsl_swap" for layer, _, _ in pairs)
    prompt, code = pairs[0][1], pairs[0][2]
    assert prompt != mechanical_prompt(row, corpus) and code != gold
    raises = parse_prompt(prompt)["raises"]
    assert not any("size" in e or "width" in e for e in raises)
    assert all(e.split(" ", 1)[1].strip("'\"") in code for e in raises)
    assert rejection(parse_prompt(prompt), code)[1:] == ([], [])


def test_a_swap_keeps_escapes_and_case_and_skips_text_outside_strings():
    import random
    import re

    from som_core.dsl_pairs import _in_strings, _swap_words, _swapped

    words = _swap_words(["Labels binary in {0, 1}"], random.Random(0))
    assert set(words) == {"Labels", "binary"} and words["Labels"][0].isupper() and words["binary"].islower()
    span = re.compile("".join(re.escape(c) if c.isalnum() else r"\\?" + re.escape(c) for c in "binary in {0, 1}"))
    swapped = span.sub(lambda m: _swapped(m.group(), words), "raises: ValueError 'binary in \\{0, 1\\}'")
    assert swapped == f"raises: ValueError '{words['binary']} in \\{{0, 1\\}}'"
    assert _in_strings('raise ValueError("binary in {0, 1}")\n', "binary in")
    assert not _in_strings('x = 1  # binary in\nraise ValueError("binary in {0, 1}")\n', "binary in")
    assert not _in_strings("x = 1\n", "binary in")


def test_drop_docstrings_keeps_code_and_fills_an_emptied_body() -> None:
    code = (
        'class Missing(Exception):\n    """Raised when absent."""\n\n\n'
        'def load(path: str) -> str:\n    """Read it.\n\n    Multi-line.\n    """\n    return path\n\n\n'
        'class Box:\n    """A box."""\n\n    def get(self) -> int:\n        """One."""\n        return 1\n'
    )
    assert drop_docstrings(code) == (
        "class Missing(Exception):\n    pass\n\n\n"
        "def load(path: str) -> str:\n    return path\n\n\n"
        "class Box:\n\n    def get(self) -> int:\n        return 1\n"
    )
    assert drop_docstrings('def f(): """same line."""\n') == 'def f(): """same line."""\n'
    assert drop_docstrings("def (\n") == "def (\n"
