"""SOM prompt: the terse, tagged request a flagship model sends to the SOM model.

Like an image-generation prompt, it is written for the model, not for a
reader: one ``tag: value`` per line, list values separated by ``; ``. The
tags, in render order:

* ``intent`` (required): what the module does, as terse clauses;
* ``exports``: names callers import from the module;
* ``calls``: one call shape per export or method, as a caller writes it —
  it fixes argument order, keywords, and how many values come back;
* ``patch``: callables tests monkeypatch — call them through these names;
* ``kwargs``: arguments passed to patched callables by keyword;
* ``asserts``: exact checks tests make on those arguments;
* ``raises``: ``Exc`` or ``Exc "match"`` pairs;
* ``attrs``, ``keywords``, ``keys``, ``values``: attribute names, keyword
  arguments, string keys and string values callers rely on;
* ``avoid``: the negative prompt — mistakes the code must not make.

``contract_fields`` extracts every tag but ``intent``/``avoid`` from a
fixture's AST (names and values only, never test bodies); that is the
mechanical prompt the training pairs start from.
"""

from __future__ import annotations

import ast
from pathlib import Path

TAGS = ("intent", "exports", "calls", "patch", "kwargs", "asserts", "raises", "attrs", "keywords", "keys", "values", "avoid", "contract")
LIST_TAGS = frozenset(TAGS) - {"intent", "contract"}
SEP = "; "


SPEC = (
    "SOM prompt: `tag: value` lines, list items joined by `; `. Tags: intent (required, at most 40 words: terse clauses naming every "
    "behavior, value, order and error the requirement fixes that no other tag already carries), exports, calls (one call per export or method as callers write it: `a, b = obj.m(x, k=v)`), patch (callables tests monkeypatch), kwargs "
    "(passed to patched callables by keyword), asserts (checks on those arguments), raises (`Exc 'match'`), attrs, "
    "keywords, keys, values, avoid (mistakes to avoid), contract (path of a contract file whose tags fill the ones you omit; never copy its tags). "
    "Copy names and messages verbatim. No prose, no code; comments and lint pragmas are dropped. "
    "To revise, send only one or two `fix: <point>` lines of at most 12 words; they join the last prompt's intent."
)


class PromptError(ValueError):
    """A prompt line the parser refuses; the message names the line."""


def render_prompt(fields: dict[str, str | list[str]]) -> str:
    """Fields -> prompt text, tags in ``TAGS`` order, empty tags dropped."""
    lines = []
    for tag in TAGS:
        value = fields.get(tag)
        if not value:
            continue
        text = SEP.join(dict.fromkeys(value)) if isinstance(value, list) else " ".join(value.split())
        lines.append(f"{tag}: {text}")
    return "\n".join(lines)


def parse_prompt(text: str) -> dict[str, str | list[str]]:
    """Prompt text -> fields; raises ``PromptError`` naming the offending line."""
    fields: dict[str, str | list[str]] = {}
    for n, line in enumerate(text.splitlines(), 1):
        if not line.strip():
            continue
        tag, colon, value = line.partition(":")
        tag = tag.strip().lower()
        if not colon or tag not in TAGS:
            raise PromptError(f"line {n}: expected `<tag>: <value>` with tag in {', '.join(TAGS)}")
        if tag in fields:
            raise PromptError(f"line {n}: duplicate tag {tag}")
        value = value.strip()
        fields[tag] = [v.strip() for v in value.split(SEP) if v.strip()] if tag in LIST_TAGS else value
    if not fields.get("intent"):
        raise PromptError("missing required tag intent")
    return fields


def expand_contract(text: str, root: Path) -> str:
    """Replace a ``contract: <path>`` line by the tags that file carries and the prompt omits.

    The file is a SOM prompt without ``intent`` (a rendered ``contract_fields``),
    resolved under ``root``; the prompt's own tags win, as in ``flagship_prompts``.
    A text without a ``contract`` line, or a ``fix:`` revision, is returned as is.
    """
    if not any(line.partition(":")[0].strip().lower() == "contract" for line in text.splitlines()):
        return text
    fields = parse_prompt(text)
    path = (root / str(fields.pop("contract"))).resolve()
    if not path.is_relative_to(root.resolve()) or not path.is_file():
        raise PromptError(f"contract: no file {path.name} in the working directory")
    given = parse_prompt("intent: -\n" + path.read_text())
    return render_prompt({**{t: v for t, v in given.items() if t not in ("intent", "avoid", "contract")}, **fields})


def fix_points(text: str) -> list[str]:
    """The points of a revision (every line ``fix: <point>``), else none."""
    lines = [line.strip() for line in text.splitlines() if line.strip()]
    if not lines or not all(line.lower().startswith("fix:") for line in lines):
        return []
    return [line[4:].strip() for line in lines]


def revise(previous: str | None, text: str) -> str:
    """Merge a revision into the last prompt: ``fix:`` lines join its intent.

    A revision is a text whose every line is ``fix: <what to change>``, so a
    flagship resends only the point the module missed, not the whole prompt.
    Any other text replaces the prompt; a revision with no previous prompt is
    refused.
    """
    fixes = fix_points(text)
    if not fixes:
        return text
    if not previous:
        raise PromptError("fix: needs a previous prompt; send the full prompt first")
    out = []
    for line in previous.splitlines():
        tag, _, value = line.partition(":")
        out.append(f"{tag}: {value.strip()}; {'; '.join(fixes)}" if tag.strip().lower() == "intent" else line)
    return "\n".join(out)


# ---------------------------------------------------------------- fixture contract


def _root(node: ast.expr) -> str:
    while isinstance(node, (ast.Attribute, ast.Call, ast.Subscript)):
        node = node.func if isinstance(node, ast.Call) else node.value
    return node.id if isinstance(node, ast.Name) else ""


_OPS = {ast.Eq: "==", ast.NotEq: "!=", ast.Is: "is", ast.IsNot: "is not", ast.In: "in"}


def contract_fields(src: str) -> dict[str, list[str]]:
    """The interface a fixture enforces: every tag but intent/avoid."""
    tree = ast.parse(src)
    modules: set[str] = set()
    exports: list[str] = []
    for n in ast.walk(tree):
        if isinstance(n, ast.Import):
            modules |= {a.asname or a.name.split(".")[0] for a in n.names}
        elif isinstance(n, ast.ImportFrom):
            if n.module == "candidate":
                exports += [a.name for a in n.names]
            else:
                modules |= {a.asname or a.name for a in n.names}
    modules.discard("candidate")
    kwargs: list[str] = []
    sink: dict[int, str] = {}
    for fn in ast.walk(tree):  # fake(**kw) reading kw.get("k") / kw["k"]
        if isinstance(fn, ast.FunctionDef) and fn.args.kwarg:
            kw = fn.args.kwarg.arg
            for c in ast.walk(fn):
                if (isinstance(c, ast.Call) and isinstance(c.func, ast.Attribute) and c.func.attr == "get"
                        and isinstance(c.func.value, ast.Name) and c.func.value.id == kw and c.args
                        and isinstance(c.args[0], ast.Constant)):
                    kwargs.append(str(c.args[0].value))
                    sink[id(c)] = str(c.args[0].value)
                elif (isinstance(c, ast.Subscript) and isinstance(c.value, ast.Name) and c.value.id == kw
                        and isinstance(c.slice, ast.Constant)):
                    kwargs.append(str(c.slice.value))
    spies: dict[str, str] = {}
    for c in ast.walk(tree):  # spy.append(kw.get("k")) -> spy holds k
        if (isinstance(c, ast.Call) and isinstance(c.func, ast.Attribute) and c.func.attr == "append"
                and c.args and id(c.args[0]) in sink and isinstance(c.func.value, ast.Name)):
            spies[c.func.value.id] = sink[id(c.args[0])]
    asserts: list[str] = []
    for c in ast.walk(tree):  # assert spy[0] == expr
        if (isinstance(c, ast.Compare) and isinstance(c.left, ast.Subscript) and isinstance(c.left.value, ast.Name)
                and c.left.value.id in spies and len(c.ops) == 1):
            asserts.append(f"{spies[c.left.value.id]} {_OPS.get(type(c.ops[0]), '?')} {ast.unparse(c.comparators[0])}")
    params = {a.arg for n in ast.walk(tree) if isinstance(n, ast.FunctionDef) for a in n.args.args} - _fixtures(tree, set(exports))
    raises: list[str] = []
    attrs: list[str] = []
    keys: list[str] = []
    values: list[str] = []
    keywords: list[str] = []
    patch: list[str] = []
    for n in ast.walk(tree):
        if isinstance(n, ast.Attribute) and _root(n) == "candidate" and isinstance(n.value, ast.Name):
            exports.append(n.attr)
        elif isinstance(n, ast.Attribute) and _root(n) not in modules | params | {"candidate"}:
            attrs.append(n.attr)
        elif isinstance(n, ast.Subscript) and isinstance(n.slice, ast.Constant) and isinstance(n.slice.value, str):
            keys.append(n.slice.value)
        elif isinstance(n, ast.Dict):
            keys += [k.value for k in n.keys if isinstance(k, ast.Constant) and isinstance(k.value, str)]
        elif isinstance(n, ast.Call):
            f = n.func
            if isinstance(f, ast.Attribute) and f.attr == "setattr" and len(n.args) >= 2:
                a0, a1 = n.args[0], n.args[1]
                patch.append(f"{ast.unparse(a0)}.{a1.value}" if isinstance(a1, ast.Constant) else ast.unparse(a0).strip("'\""))
            if isinstance(f, ast.Attribute) and f.attr == "raises" and n.args:
                m = next((k.value for k in n.keywords if k.arg == "match"), None)
                raises.append(ast.unparse(n.args[0]) + (f" {ast.unparse(m)}" if m is not None else ""))
            elif _root(n) not in modules:
                keywords += [k.arg for k in n.keywords if k.arg]
        elif isinstance(n, ast.Assert):
            for c in ast.walk(n.test):
                if isinstance(c, ast.Compare):
                    for side in [c.left, *c.comparators]:
                        if isinstance(side, ast.Constant) and isinstance(side.value, str):
                            values.append(repr(side.value))
    out = {"exports": exports, "calls": _calls(tree, set(exports)), "patch": patch, "kwargs": kwargs, "asserts": asserts, "raises": raises,
           "attrs": attrs, "keywords": keywords, "keys": keys, "values": values}
    return {k: list(dict.fromkeys(v)) for k, v in out.items() if v}


CALL_CHARS = 100


def _fixtures(tree: ast.Module, exports: set[str]) -> set[str]:
    """Pytest fixtures that return or yield an export's result: a test's
    ``parser`` parameter is then the export's instance, not a local fake."""
    names: set[str] = set()
    for fn in ast.walk(tree):
        if isinstance(fn, ast.FunctionDef) and any("fixture" in ast.unparse(d) for d in fn.decorator_list):
            for n in ast.walk(fn):
                if isinstance(n, (ast.Return, ast.Yield)) and isinstance(n.value, ast.Call) and _root(n.value) in exports:
                    names.add(fn.name)
    return names


def _calls(tree: ast.Module, exports: set[str]) -> list[str]:
    """First call shape per callee: an export, or a method on a name bound to
    an export's result (``c = Cipher(k)`` then ``c.encrypt(...)``)."""
    bound = _fixtures(tree, exports)
    for n in ast.walk(tree):
        if (isinstance(n, ast.Assign) and isinstance(n.value, ast.Call) and _root(n.value) in exports
                and all(isinstance(t, ast.Name) for t in n.targets)):
            bound |= {t.id for t in n.targets}
    shapes: dict[str, str] = {}
    for n in ast.walk(tree):
        if isinstance(n, (ast.Assign, ast.Expr)) and isinstance(n.value, (ast.Call, ast.Await)):
            call = n.value.value if isinstance(n.value, ast.Await) else n.value
            if not isinstance(call, ast.Call) or _root(call.func) not in exports | bound:
                continue
            key = ast.unparse(call.func)
            text = ast.unparse(n)
            if key not in shapes and len(text) <= CALL_CHARS and "; " not in text:
                shapes[key] = text
    return list(shapes.values())
