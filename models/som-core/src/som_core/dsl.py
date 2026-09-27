"""Compile SOM DSL — terse Python — into one finished, gate-clean module.

The DSL is Python minus the parts a caller should not spend tokens on:

* no imports: every free name that the training corpus binds by an import
  (``jwt``, ``BaseModel``, ``HTTPException``...) is imported automatically;
  an explicit import still wins, so a caller can disambiguate;
* no layout: one-line compounds (``if x: raise E("m")``) and any spacing are
  accepted; ``ruff check --fix`` and ``ruff format`` under the corpus
  quality config produce the final text;
* no module docstring: ``doc`` becomes the module docstring.

Before anything is written the near-miss rules (``RULES``) run on the AST:
each is a failure mode a curated near-miss candidate was caught making,
rejected with a one-line reason. ``compile_dsl`` returns
``(source, diagnostics)``; a non-empty diagnostics list means nothing was
produced. The import table is built from non-holdout families only.
"""

from __future__ import annotations

import ast
import json
import os
import re
import subprocess
import sys
import tempfile
import tomllib
from collections import Counter, defaultdict
from functools import cache
from pathlib import Path
from typing import Any

from som_core.paths import ROOT

PYTHON_PROJECT = ROOT.parent / "som-code-python"
CONFIG = PYTHON_PROJECT / "pyproject.toml"


def _ruff() -> str:
    return os.environ.get("SOM_RUFF") or str(PYTHON_PROJECT / ".venv" / "bin" / "ruff")


@cache
def import_table() -> dict[str, str]:
    """Bound name -> the import statement the non-holdout corpus uses most for it."""
    from som_core.dataset import load_corpora
    from som_core.train import HOLDOUT

    votes: dict[str, Counter[str]] = defaultdict(Counter)
    for row in load_corpora():
        if row["id"] in HOLDOUT or "decompiled" not in row["metadata"]:
            continue
        for op in row["metadata"]["decompiled"]["ops"]:
            if op["op"] != "ADD_IMPORT":
                continue
            node = ast.parse(op["stmt"]).body[0]
            if isinstance(node, ast.Import):
                for a in node.names:
                    name = a.asname or a.name.split(".")[0]
                    votes[name][f"import {a.name}" + (f" as {a.asname}" if a.asname else "")] += 1
            elif isinstance(node, ast.ImportFrom) and node.module and node.level == 0:
                for a in node.names:
                    votes[a.asname or a.name][f"from {node.module} import {a.name}" + (f" as {a.asname}" if a.asname else "")] += 1
    table = {name: c.most_common(1)[0][0] for name, c in votes.items()}
    for mod in sys.stdlib_module_names:  # any stdlib module a caller names bare
        if not mod.startswith("_"):
            table.setdefault(mod, f"import {mod}")
    return table


# ---------------------------------------------------------------- near-miss rules


def _kw(call: ast.Call, name: str) -> ast.expr | None:
    return next((k.value for k in call.keywords if k.arg == name), None)


def _dotted(node: ast.expr) -> str:
    if isinstance(node, ast.Name):
        return node.id
    if isinstance(node, ast.Attribute):
        return f"{_dotted(node.value)}.{node.attr}"
    return ""


def _is_const(node: ast.expr | None, value: object) -> bool:
    return isinstance(node, ast.Constant) and node.value is value


SECRETISH = re.compile(r"(token|secret|password|passwd|signature|hmac|api_?key|mac)$", re.I)


def _names(node: ast.expr) -> str:
    return _dotted(node) or (node.func.attr if isinstance(node, ast.Call) and isinstance(node.func, ast.Attribute) else "")


def _punct_ranges(pattern: str) -> list[tuple[str, str]]:
    """Character-class ranges whose ends are both punctuation or space: ``[ -()]``
    meant space, hyphen, parens, but spans ``' '..'('`` and leaves ``)`` out
    (e2e-010 family 14). No gold module writes one."""
    from re import _parser

    try:
        stack, found = [_parser.parse(pattern)], []
    except re.error:
        return []
    while stack:
        for op, av in stack.pop():
            if op == _parser.IN:
                found += [(chr(a), chr(b)) for o, (a, b) in ((o, x) for o, x in av if o == _parser.RANGE)
                          if not chr(a).isalnum() and not chr(b).isalnum()]
            else:
                for x in av if isinstance(av, (list, tuple)) else ():
                    stack += [x] if isinstance(x, _parser.SubPattern) else [y for y in x if isinstance(y, _parser.SubPattern)] if isinstance(x, (list, tuple)) else []
    return found


def _check_call(call: ast.Call) -> list[str]:
    out: list[str] = []
    fn = _dotted(call.func)
    tail = fn.rsplit(".", 1)[-1]
    if fn in ("yaml.load", "yaml.load_all", "yaml.unsafe_load"):
        loader = _kw(call, "Loader") or (call.args[1] if len(call.args) > 1 else None)
        if fn == "yaml.unsafe_load" or loader is None or _dotted(loader).rsplit(".", 1)[-1] in ("Loader", "UnsafeLoader", "FullLoader"):
            out.append(f"{fn}: unsafe loader executes tags; use yaml.safe_load or a SafeLoader subclass")
    if fn in ("jwt.decode",):
        opts = _kw(call, "options")
        off = {str(k.value) for k, v in zip(opts.keys, opts.values) if isinstance(k, ast.Constant) and _is_const(v, False)} if isinstance(opts, ast.Dict) else set()
        if "verify_signature" not in off:  # an unverified peek (e.g. to pick the key by `iss`) checks nothing, so nothing is disabled
            if _kw(call, "algorithms") is None:
                out.append("jwt.decode without algorithms=[...] accepts attacker-chosen algorithms")
            out += [f"jwt.decode options {k}=False disables a required check" for k in sorted(off) if k.startswith("verify_")]
    if tail in ("XMLParser",) and _is_const(_kw(call, "resolve_entities"), True):
        out.append("XMLParser(resolve_entities=True) enables XXE")
    if tail in ("XMLParser",) and _is_const(_kw(call, "no_network"), False):
        out.append("XMLParser(no_network=False) lets entities fetch URLs")
    if tail == "ECB" or fn.endswith("modes.ECB"):
        out.append("ECB mode leaks plaintext structure; use an AEAD (AESGCM) or CBC/CTR with random IV")
    if tail == "PBKDF2HMAC":
        it = _kw(call, "iterations")
        if isinstance(it, ast.Constant) and isinstance(it.value, int) and it.value < 600_000:
            out.append(f"PBKDF2 iterations={it.value} below the OWASP 600000 floor")
    if (tail in ("MD5", "SHA1") and "hashes" in fn) or fn in ("hashlib.md5", "hashlib.sha1"):
        out.append(f"{fn}: broken hash for security use")
    if fn in ("pickle.loads", "pickle.load", "eval", "exec", "marshal.loads"):
        out.append(f"{fn} on untrusted input executes code")
    if fn.startswith("re.") and call.args and isinstance(call.args[0], ast.Constant) and isinstance(call.args[0].value, str):
        out += [f"{fn}: [{a}-{b}] is a range, not the characters {a!r}, '-', {b!r}; escape that hyphen: [{a}\\-{b}...] not [{a}-{b}...]"
                for a, b in _punct_ranges(call.args[0].value)]
    if fn.startswith(("requests.", "httpx.")) and tail in ("get", "post", "put", "patch", "delete", "request") and _is_const(_kw(call, "verify"), False):
        out.append(f"{fn}(verify=False) disables TLS verification")
    if tail in ("run", "Popen", "call", "check_output", "check_call") and fn.startswith("subprocess") and _is_const(_kw(call, "shell"), True):
        out.append(f"{fn}(shell=True) enables command injection")
    return out


def near_miss(tree: ast.Module) -> list[str]:
    """Reasons ``tree`` repeats a failure mode the curated near misses were caught on."""
    out: list[str] = []
    for node in ast.walk(tree):
        line = getattr(node, "lineno", 0)
        found: list[str] = []
        if isinstance(node, ast.Call):
            found += _check_call(node)
        elif isinstance(node, ast.Compare) and len(node.ops) == 1 and isinstance(node.ops[0], (ast.Eq, ast.NotEq)):
            sides = [_names(node.left), _names(node.comparators[0])]
            if all(sides) and any(SECRETISH.search(s.rsplit(".", 1)[-1]) for s in sides if s):
                found.append("== on a secret leaks timing; use hmac.compare_digest")
        elif isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
            for d in node.args.defaults + [d for d in node.args.kw_defaults if d is not None]:
                if isinstance(d, (ast.List, ast.Dict, ast.Set)) or (isinstance(d, ast.Call) and _dotted(d.func) in ("list", "dict", "set")):
                    found.append(f"{node.name}: mutable default argument is shared across calls; default to None")
        elif isinstance(node, ast.ExceptHandler) and node.type is None:
            found.append("bare except swallows KeyboardInterrupt/SystemExit; name the exception")
        out += [f"L{line}: {m}" for m in found if m]
    calls = [n for n in ast.walk(tree) if isinstance(n, ast.Call)]
    kept = [c for c in calls if _dotted(c.func).endswith("mkstemp")
            or (_dotted(c.func).endswith("NamedTemporaryFile") and _is_const(_kw(c, "delete"), False))]
    if kept and not any(_dotted(c.func).rsplit(".", 1)[-1] in ("unlink", "remove") for c in calls):
        out.append(f"L{kept[0].lineno}: a kept temp file (delete=False/mkstemp) is never unlinked; "
                   "wrap the write and replace in try, and unlink the temp file in except before re-raising")
    return out


# ---------------------------------------------------------------- compile


def _ruff_json(args: list[str], source: str) -> tuple[str, list[dict[str, Any]]]:
    with tempfile.TemporaryDirectory(prefix="som-dsl") as tmp:
        path = Path(tmp) / "candidate.py"
        path.write_text(source)
        proc = subprocess.run([_ruff(), *args, "--config", str(CONFIG), "--no-cache", "--output-format", "json", str(path)],
                              capture_output=True, text=True, check=False)
        return path.read_text(), json.loads(proc.stdout or "[]")


def _undefined(source: str) -> list[str]:
    _, diags = _ruff_json(["check", "--select", "F821", "--exit-zero"], source)
    return list(dict.fromkeys(re.search(r"`([^`]+)`", d["message"]).group(1) for d in diags if d["code"] == "F821"))


def _hoist(code: str) -> str:
    """Move ``engine = create_engine(...)`` above the top-level statement that reads it first.

    Only a single-name assignment moves, and only when its value reads nothing the
    module defines at or after the new position; anything else is left for the rejection.
    """
    for _ in range(8):
        tree = ast.parse(code)
        _, diags = _ruff_json(["check", "--select", "F821", "--exit-zero"], code)
        body, moved = tree.body, False
        index_of = lambda row: next((i for i, n in enumerate(body) if n.lineno <= row <= (n.end_lineno or n.lineno)), None)
        for d in diags:
            name, use = re.search(r"`([^`]+)`", d["message"]).group(1), index_of(d["location"]["row"])
            src = next((i for i, n in enumerate(body) if isinstance(n, ast.Assign) and len(n.targets) == 1
                        and isinstance(n.targets[0], ast.Name) and n.targets[0].id == name), None)
            if use is None or src is None or src <= use:
                continue
            later = set().union(*(_bound(ast.Module(body=[n], type_ignores=[])) for n in body[use:src]))
            if {x.id for x in ast.walk(body[src].value) if isinstance(x, ast.Name)} & later:
                continue
            lines = code.splitlines(keepends=True)
            block = lines[body[src].lineno - 1 : body[src].end_lineno]
            del lines[body[src].lineno - 1 : body[src].end_lineno]
            at = min([body[use].lineno - 1] + [dec.lineno - 1 for dec in getattr(body[use], "decorator_list", [])])
            code = "".join(lines[:at] + block + ["\n"] + lines[at:])
            moved = True
            break
        if not moved:
            return code
    return code


def _imported(tree: ast.Module) -> set[str]:
    return {a.asname or a.name.split(".")[0] for n in tree.body if isinstance(n, (ast.Import, ast.ImportFrom)) for a in n.names}


def _bound(tree: ast.Module) -> set[str]:
    names = _imported(tree)
    for node in tree.body:
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)):
            names.add(node.name)
        elif isinstance(node, (ast.Assign, ast.AnnAssign)):
            for t in node.targets if isinstance(node, ast.Assign) else [node.target]:
                names |= {n.id for n in ast.walk(t) if isinstance(n, ast.Name)}
    return names


def _from_packages(names: list[str], imports: list[str]) -> dict[str, str]:
    """Resolve ``names`` as attributes of the packages the module already imports.

    Runs in the corpus interpreter, where the third-party libraries live.
    """
    mods = sorted({m.split()[1].split(".")[0] for m in imports})
    if not names or not mods:
        return {}
    probe = (
        "import importlib, json, sys\n"
        "names, mods = json.loads(sys.argv[1]), json.loads(sys.argv[2])\n"
        "out = {}\n"
        "for m in mods:\n"
        "    try: mod = importlib.import_module(m)\n"
        "    except Exception: continue\n"
        "    for n in names:\n"
        "        if n not in out and hasattr(mod, n): out[n] = f'from {m} import {n}'\n"
        "print(json.dumps(out))\n"
    )
    python = PYTHON_PROJECT / ".venv" / "bin" / "python"
    proc = subprocess.run([str(python), "-c", probe, json.dumps(names), json.dumps(mods)],
                          capture_output=True, text=True, check=False, timeout=60)
    return json.loads(proc.stdout or "{}") if proc.returncode == 0 else {}


_INDEX_PROBE = """
import importlib, inspect, json, pkgutil, sys, warnings
warnings.simplefilter("ignore")
from importlib.metadata import packages_distributions
dists = {d.lower().replace("_", "-") for d in json.loads(sys.argv[2])}
tops = set(json.loads(sys.argv[1])) | {t for t, ds in packages_distributions().items()
                                        if not t.startswith("_") and any(d.lower().replace("_", "-") in dists for d in ds)}
out = {}
for top in sorted(tops):
    try:
        pkg = importlib.import_module(top)
    except Exception:
        continue
    names = [top] + [m.name for m in pkgutil.walk_packages(getattr(pkg, "__path__", []), top + ".", onerror=lambda _: None)]
    for modname in names:
        if any(p.startswith("_") or p in ("tests", "testing", "conftest") for p in modname.split(".")[1:]):
            continue
        try:
            mod = importlib.import_module(modname)
        except BaseException:
            continue
        for n, obj in vars(mod).items():
            if n.startswith("_") or not (inspect.isclass(obj) or inspect.isfunction(obj)):
                continue
            if n not in out or modname.count(".") < out[n].count("."):
                out[n] = modname
print(json.dumps({n: f"from {m} import {n}" for n, m in out.items()}))
"""


@cache
def symbol_index() -> dict[str, str]:
    """Public class/function name -> its shallowest import, over the corpus's third-party packages.

    The auto-import of last resort, for a name no training family imports
    (e.g. ``AESGCM``, ``redis.WatchError``). Packages are those the corpus imports plus
    the corpus project's declared dependencies; built once in the corpus interpreter and cached on disk.
    """
    packages = sorted({v.split()[1].split(".")[0] for v in import_table().values()} - set(sys.stdlib_module_names))
    declared = sorted(re.match(r"[A-Za-z0-9_.-]+", d).group(0) for d in tomllib.loads((PYTHON_PROJECT / "pyproject.toml").read_text())["project"]["dependencies"])
    packages = packages + [f"dist:{d}" for d in declared]
    cache_file = ROOT / ".cache" / "symbol_index.json"
    cached = json.loads(cache_file.read_text()) if cache_file.is_file() else {}
    if cached.get("packages") == packages:
        return cached["index"]
    python = PYTHON_PROJECT / ".venv" / "bin" / "python"
    proc = subprocess.run([str(python), "-c", _INDEX_PROBE, json.dumps([p for p in packages if not p.startswith("dist:")]), json.dumps(declared)],
                          capture_output=True, text=True, check=False, timeout=900)
    index = json.loads(proc.stdout.strip().splitlines()[-1]) if proc.returncode == 0 and proc.stdout.strip() else {}
    cache_file.parent.mkdir(parents=True, exist_ok=True)
    cache_file.write_text(json.dumps({"packages": packages, "index": index}))
    return index


_ATTR_PROBE = """
import importlib, json, sys, warnings
warnings.simplefilter("ignore")
out = []
for mod, chain in json.loads(sys.argv[1]):
    try:
        obj = importlib.import_module(mod)
    except Exception:
        continue
    for i, part in enumerate(chain):
        if not hasattr(obj, part):
            try:
                obj = importlib.import_module(".".join([mod, *chain[: i + 1]]))
                continue
            except Exception:
                out.append(".".join([mod, *chain[: i + 1]]))
                break
        obj = getattr(obj, part)
        if type(obj).__name__ != "module":
            break
print(json.dumps(out))
"""


def _absent_attributes(tree: ast.Module) -> list[str]:
    """``yaml.NonScalarNode`` read through an ``import yaml`` binding, or ``from sqlalchemy import Base``, that the module does not have.

    ``from pkg import sub`` binds ``sub`` the same way when ``pkg.sub`` imports as a module
    (``algorithms.AESGCM``); a class or function bound that way is skipped. Probed in the corpus interpreter, following the dotted chain only while it stays a module;
    a submodule that imports on demand counts as present.
    """
    alias = {a.asname or a.name.split(".")[0]: (a.name if a.asname else a.name.split(".")[0])
             for n in tree.body if isinstance(n, ast.Import) for a in n.names}
    alias |= {a.asname or a.name: f"{n.module}.{a.name}" for n in tree.body
              if isinstance(n, ast.ImportFrom) and n.module and not n.level for a in n.names if a.name != "*"}
    chains = set()
    for node in ast.walk(tree):
        if not isinstance(node, ast.Attribute) or not isinstance(node.ctx, ast.Load):
            continue
        parts, cur = [], node
        while isinstance(cur, ast.Attribute):
            parts.insert(0, cur.attr)
            cur = cur.value
        if isinstance(cur, ast.Name) and cur.id in alias:
            chains.add((alias[cur.id], tuple(parts)))
    chains |= {(n.module, (a.name,)) for n in tree.body if isinstance(n, ast.ImportFrom) and n.module and not n.level for a in n.names if a.name != "*"}
    chains = {c for c in chains if not any(o[0] == c[0] and len(o[1]) > len(c[1]) and o[1][: len(c[1])] == c[1] for o in chains)}
    if not chains:
        return []
    python = PYTHON_PROJECT / ".venv" / "bin" / "python"
    proc = subprocess.run([str(python), "-c", _ATTR_PROBE, json.dumps(sorted(chains))],
                          capture_output=True, text=True, check=False, timeout=120)
    return json.loads(proc.stdout.strip().splitlines()[-1]) if proc.returncode == 0 and proc.stdout.strip() else []


_SUBSCRIPT_PROBE = """
import importlib, json, sys, warnings
warnings.simplefilter("ignore")
out = []
for mod, chain in json.loads(sys.argv[1]):
    try:
        obj = importlib.import_module(mod)
        for part in chain:
            obj = getattr(obj, part)
    except Exception:
        continue
    if not (hasattr(obj, "__class_getitem__") or hasattr(type(obj), "__getitem__")):
        out.append(".".join([mod, *chain]))
print(json.dumps(out))
"""


def _eager_annotations(tree: ast.Module) -> list[ast.expr]:
    """Annotations Python evaluates at definition time: every parameter and return, plus module- and class-level ``x: T``."""
    out: list[ast.expr] = []
    bodies = [tree.body]
    while bodies:  # x: T in a function body is never evaluated; at module or class level it is
        for node in bodies.pop():
            if isinstance(node, ast.AnnAssign):
                out.append(node.annotation)
            elif isinstance(node, ast.ClassDef):
                bodies.append(node.body)
    for node in ast.walk(tree):
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
            a = node.args
            out.extend(x.annotation for x in [*a.posonlyargs, *a.args, *a.kwonlyargs, a.vararg, a.kwarg] if x and x.annotation)
            if node.returns:
                out.append(node.returns)
    return out


def _unsubscriptable(tree: ast.Module) -> list[str]:
    """``pd.Series`` in ``-> pd.Series[float]``: an imported object an eager annotation subscripts that has no runtime ``[]``.

    Probed in the corpus interpreter, like :func:`_absent_attributes`; names the module defines itself are never probed.
    """
    bound: dict[str, tuple[str, tuple[str, ...]]] = {  # name -> (module, attribute chain inside it)
        a.asname or a.name.split(".")[0]: (a.name if a.asname else a.name.split(".")[0], ())
        for n in tree.body if isinstance(n, ast.Import) for a in n.names}
    bound |= {a.asname or a.name: (n.module, (a.name,)) for n in tree.body
              if isinstance(n, ast.ImportFrom) and n.module and not n.level for a in n.names if a.name != "*"}
    chains = set()
    for ann in _eager_annotations(tree):
        for sub in ast.walk(ann):
            if not isinstance(sub, ast.Subscript):
                continue
            parts, cur = [], sub.value
            while isinstance(cur, ast.Attribute):
                parts.insert(0, cur.attr)
                cur = cur.value
            if isinstance(cur, ast.Name) and cur.id in bound:
                mod, head = bound[cur.id]
                chains.add((mod, (*head, *parts)))
    if not chains:
        return []
    python = PYTHON_PROJECT / ".venv" / "bin" / "python"
    proc = subprocess.run([str(python), "-c", _SUBSCRIPT_PROBE, json.dumps(sorted(chains))],
                          capture_output=True, text=True, check=False, timeout=120)
    return json.loads(proc.stdout.strip().splitlines()[-1]) if proc.returncode == 0 and proc.stdout.strip() else []


FUTURE = "from __future__ import annotations"


def compile_dsl(code: str, doc: str = "", exports: list[str] | None = None) -> tuple[str, list[str]]:
    """Return (module source, diagnostics); diagnostics non-empty means rejected.

    ``exports`` are names the module must bind at top level; one the import
    table knows is imported (a re-export), any other missing one is rejected.
    """
    try:
        tree = ast.parse(code)
        compile(tree, "<dsl>", "exec")  # the symbol-table errors ast.parse lets through (duplicate argument, ...)
    except SyntaxError as exc:
        return "", [f"L{exc.lineno}: syntax: {exc.msg}"]
    diags = near_miss(tree)
    if diags:
        return "", diags
    future = [n for n in tree.body if isinstance(n, ast.ImportFrom) and n.module == "__future__"]
    if future:  # hoisted above the auto-imports, where Python requires it
        lines = code.splitlines(keepends=True)
        for n in future:
            lines[n.lineno - 1 : (n.end_lineno or n.lineno)] = [""] * ((n.end_lineno or n.lineno) - n.lineno + 1)
        code = "".join(lines)
        tree = ast.parse(code)
    code = _hoist(code)
    tree = ast.parse(code)
    table = import_table()
    missing = _undefined(code)
    absent = [n for n in exports or [] if n not in _bound(tree) and n not in missing]
    missing += absent
    defined = _bound(tree)  # used above its own top-level definition: never a library name
    imports = [table[n] for n in missing if n in table and n not in defined]
    written = [ast.unparse(n) for n in tree.body if isinstance(n, (ast.Import, ast.ImportFrom))]
    found = _from_packages([n for n in missing if n not in table and n not in defined], imports + written)
    imports += [found[n] for n in missing if n in found]
    index = symbol_index() if any(n not in table and n not in found and n not in defined for n in missing) else {}
    found |= {n: index[n] for n in missing if n not in table and n not in found and n in index and n not in absent and n not in defined}
    imports += [index[n] for n in missing if n in index and found.get(n) == index[n] and index[n] not in imports]
    rest = [n for n in missing if n not in table and n not in found and n not in defined and n not in absent]
    roots = {v.split()[1].split(".")[0] for v in (index or symbol_index()).values()} if rest else set()
    bare = {n: f"import {n}" for n in rest if n in roots}
    found |= bare  # an indexed package named bare, e.g. ``redis.WatchError``
    imports += list(bare.values())
    unknown = [n for n in missing if (n not in table or n in defined) and n not in found]
    if unknown:  # names that vanish once imports exist were string annotations, e.g. Literal["a"]
        still = set(_undefined("\n".join(dict.fromkeys(imports + written)) + "\n" + code))
        unknown = [n for n in unknown if n in still or n in absent]  # an absent export is never referenced
    forward = [n for n in unknown if n in defined]
    if forward:  # a forward reference in an annotation, which postponed annotations resolve
        early = set(_undefined(FUTURE + "\n" + "\n".join(dict.fromkeys(imports + written)) + "\n" + code))
        if early & set(forward):
            return "", [f"used above its module-level definition (move the definition up): {', '.join(n for n in forward if n in early)}"]
        imports.insert(0, FUTURE)
        unknown = [n for n in unknown if n not in forward]
    lost = [n for n in unknown if n in absent]
    if lost:  # the model dropped an export; name it as one, not as a stray reference
        return "", [f"exports lists {n} but the module never defines it; keep every earlier definition and add {n}" for n in lost]
    if unknown:
        return "", [f"undefined, not in the auto-import table (import it explicitly or define it): {', '.join(unknown)}"]
    imports = [ast.unparse(n) for n in future] + [i for i in imports if i != FUTURE or not future]
    head = f'"""{doc.strip() or "Generated module."}"""\n\n' if not ast.get_docstring(tree) else ""
    reexport = [n for n in exports or [] if n in absent or n in _imported(tree)]
    tail = f"\n__all__ = {sorted(set(exports or []))!r}\n" if reexport else ""
    try:
        source, final = _finish(head, imports, code, tail)
        if FUTURE not in imports and _unsubscriptable(final):  # -> pd.Series[float] would raise TypeError at import
            source, final = _finish(head, [FUTURE, *imports], code, tail)
    except SyntaxError as exc:  # a fix that broke the module is our bug, not the caller's
        return "", [f"internal: formatter produced invalid code at L{exc.lineno}"]
    absent_attrs = _absent_attributes(final)
    if absent_attrs:
        return "", [f"no such attribute in the installed package: {', '.join(absent_attrs)}"]
    return source, []


def _finish(head: str, imports: list[str], code: str, tail: str) -> tuple[str, ast.Module]:
    """Assemble, ruff-fix, and format the module; its source and parse tree."""
    source = head + "\n".join(dict.fromkeys(imports)) + "\n\n" + code + tail
    source, _ = _ruff_json(["check", "--fix", "--unsafe-fixes", "--exit-zero"], source)
    with tempfile.TemporaryDirectory(prefix="som-fmt") as tmp:
        path = Path(tmp) / "candidate.py"
        path.write_text(source)
        subprocess.run([_ruff(), "format", "--config", str(CONFIG), "--no-cache", str(path)], capture_output=True, check=False)
        source = path.read_text()
    return source, ast.parse(source)


def _import_lines(node: ast.Import | ast.ImportFrom) -> list[tuple[str, str]]:
    """(bound name, single-name import statement) for each alias of ``node``."""
    if isinstance(node, ast.Import):
        return [(a.asname or a.name.split(".")[0], f"import {a.name}" + (f" as {a.asname}" if a.asname else "")) for a in node.names]
    mod = "." * node.level + (node.module or "")
    return [(a.asname or a.name, f"from {mod} import {a.name}" + (f" as {a.asname}" if a.asname else "")) for a in node.names]


def drop_docstrings(code: str) -> str:
    """``code`` without its function and class docstrings, the free text no fixture reads;
    a body left empty gets ``pass``. A docstring sharing a line with other code stays,
    and so does all of ``code`` when it does not parse."""
    try:
        tree = ast.parse(code)
    except SyntaxError:
        return code
    lines = code.splitlines(keepends=True)
    edits = []
    for node in ast.walk(tree):
        if not isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)):
            continue
        first = node.body[0]
        if not (isinstance(first, ast.Expr) and isinstance(first.value, ast.Constant) and isinstance(first.value.value, str)):
            continue
        end = first.end_lineno or first.lineno
        if first.lineno == node.lineno or lines[first.lineno - 1][: first.col_offset].strip() or lines[end - 1][first.end_col_offset or 0 :].strip():
            continue
        edits.append((first.lineno - 1, end, [] if len(node.body) > 1 else [" " * first.col_offset + "pass\n"]))
    for start, end, keep in sorted(edits, reverse=True):
        lines[start:end] = keep
    return "".join(lines)


def to_dsl(source: str) -> tuple[str, str]:
    """Finished module -> (DSL code, doc): the inverse of ``compile_dsl``.

    Drops the module docstring (returned as ``doc``) and every top-level
    import the auto-import table would re-derive; an import the table spells
    differently stays, since an explicit import wins at compile time.
    """
    tree = ast.parse(source)
    table = import_table()
    doc = ast.get_docstring(tree) or ""
    lines = source.splitlines(keepends=True)
    drop: set[int] = set()
    for i, node in enumerate(tree.body):
        if i == 0 and doc and isinstance(node, ast.Expr):
            drop |= set(range(node.lineno - 1, node.end_lineno or node.lineno))
        elif isinstance(node, (ast.Import, ast.ImportFrom)) and not (isinstance(node, ast.ImportFrom) and node.module == "__future__"):
            pairs = _import_lines(node)
            if all(table.get(name) == stmt for name, stmt in pairs):
                drop |= set(range(node.lineno - 1, node.end_lineno or node.lineno))
            else:
                kept = [stmt for name, stmt in pairs if table.get(name) != stmt]
                lines[node.lineno - 1] = "\n".join(kept) + "\n"
                drop |= set(range(node.lineno, node.end_lineno or node.lineno))
    code = "".join(line for n, line in enumerate(lines) if n not in drop)
    if FUTURE in code:  # compile re-adds it exactly when an annotation names a later definition
        bare = code.replace(FUTURE + "\n", "", 1)
        if set(_undefined(bare)) & _bound(ast.parse(bare)):
            code = bare
    return re.sub(r"\n{3,}", "\n\n", code).strip() + "\n", doc
