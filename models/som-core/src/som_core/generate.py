"""SOM prompt -> DSL -> module: the SOM model's side of ``som_generate``.

The model reads a SOM prompt (``som_core.prompt``) and writes DSL — Python
without imports, module docstring, or layout care (``som_core.dsl``).
``compile_dsl`` turns it into the finished module; when it rejects, its
diagnostics go back to the model as the next user turn, up to ``RETRIES``
attempts. The fixture is never consulted and only one candidate is kept.
"""

from __future__ import annotations

import ast
import re
from pathlib import Path
from typing import Any

from som_core.dsl import compile_dsl
from som_core.prompt import parse_prompt

SYSTEM_DSL = (
    "You translate a SOM prompt into one Python module written as SOM DSL: plain Python without imports, "
    "without a module docstring, formatting free. Define every name in `exports`, call patched callables "
    "through the names in `patch`, pass `kwargs` by keyword, and never do what `avoid` lists. Reply with the code only."
)
RETRIES = 3
MAX_TOKENS = 2048
FENCE = re.compile(r"```(?:python|py)?\n(.*?)(?:```|\Z)", re.S)
SINGLE_QUOTED = re.compile(r"(?<![\w'\"])([rbfRBF]{0,2})'((?:[^'\"\\\n]|\\.)*)'(?!\w)")


def double_quote(prompt: str) -> str:
    """``r'^\\d$'`` -> ``r"^\\d$"``. The DSL is ruff-formatted, so its strings
    are double-quoted; given a single-quoted raw literal, the model copied the
    pattern but dropped the quote (``re.compile(r^\\d$)``, base and adapter
    alike). Literals holding a double quote, and apostrophes in words, stay."""
    return SINGLE_QUOTED.sub(r'\1"\2"', prompt)


def dsl_messages(prompt: str, completion: str | None = None) -> list[dict[str, str]]:
    msgs = [{"role": "system", "content": SYSTEM_DSL}, {"role": "user", "content": double_quote(prompt)}]
    return msgs + ([{"role": "assistant", "content": completion}] if completion is not None else [])


def revision_messages(prompt: str, attempt: str, feedback: list[str], completion: str | None = None) -> list[dict[str, str]]:
    """One retry turn: the rejected ``attempt`` and what it missed. ``generate_module``
    retries in this shape and ``dsl_pairs.fix_pairs`` trains on it, so the two agree."""
    msgs = dsl_messages(prompt) + [
        {"role": "assistant", "content": attempt},
        {"role": "user", "content": "rejected:\n" + "\n".join(feedback) + "\nReply with the corrected code only."},
    ]
    return msgs + ([{"role": "assistant", "content": completion}] if completion is not None else [])


def strip_fence(text: str) -> str:
    m = FENCE.search(text)
    return (m.group(1) if m else text).strip() + "\n"


def load_model(base: Path | str, adapter: Path | str | None = None):
    from mlx_lm import load

    model, tokenizer = load(str(base), adapter_path=str(adapter) if adapter else None)
    tokenizer.add_eos_token(tokenizer.eos_token)  # the chat end-of-turn, missing from the locked eos ids
    return model, tokenizer


def _complete(model, tokenizer, msgs: list[dict[str, str]], max_tokens: int = MAX_TOKENS) -> str:
    """Greedy, guarded only against an 80-token loop. No repetition penalty:
    code repeats its identifiers, and a penalty over the last few tokens bends
    the second use of a name into a new one (``TTL`` -> ``TTL_TTL``)."""
    from mlx_lm import generate

    from som_core.evaluate import NO_REPEAT_NGRAM, no_repeat_ngram

    chat = tokenizer.apply_chat_template(msgs, add_generation_prompt=True, return_dict=False)
    return generate(model, tokenizer, chat, max_tokens=max_tokens, logits_processors=[no_repeat_ngram(NO_REPEAT_NGRAM, len(chat))])


DEFAULT = re.compile(r"default(?:ing|s)? (?:to |of )?(-?\d[\d_.,]*\d|\d)")


def conformance(fields: dict[str, Any], code: str) -> list[str]:
    """What the prompt fixes that the DSL visibly lacks, checked against the
    prompt alone, never the fixture: each number the intent gives as a default
    must be written somewhere, each literal ``Name(k=v)`` the intent gives for
    an export must be a defaulted parameter, and each ``kwargs`` name passed
    by keyword to the patched callee, and each worded ``raises`` message the
    module raises its own exception type for (``_raises_misses``). Not keys: a ``keys``
    string can come from the caller's own fakes and data, so a correct module
    may lack it (188 of 300 gold modules would be flagged)."""
    digits = re.sub(r"(?<=\d)_(?=\d)", "", code)
    return [
        f"intent: default {num} is never written"
        for num in DEFAULT.findall(str(fields.get("intent", "")))
        if not re.search(rf"(?<![\w.]){re.escape(re.sub(r"[_,]", "", num))}(?![\w])", digits)
    ] + _signature_misses(fields, code) + _keyword_misses(fields, code) + _raises_misses(fields, code)


RAISES = re.compile(r"^(\w[\w.]*)\s+(['\"])(.*)\2$")
FORMAT = re.compile(r"%[-\d.]*[sdrfi]|\{[\w.\[\]]*(?:![rsa])?(?::[^{}]*)?\}")


def _string_shapes(tree: ast.AST) -> list[list[str | None]]:
    """Every string the module can build, as literal pieces with ``None``
    where an f-string field, ``%s`` or ``{}`` placeholder goes."""
    shapes = []
    for n in ast.walk(tree):
        if isinstance(n, ast.JoinedStr):
            pieces = [v.value if isinstance(v, ast.Constant) else None for v in n.values]
        elif isinstance(n, ast.Constant) and isinstance(n.value, str):
            pieces = []
            for i, piece in enumerate(FORMAT.split(n.value)):
                pieces += [None, piece] if i else [piece]
        else:
            continue
        shapes.append([p for p in pieces if p != ""])
    return shapes


def _produces(shape: list[str | None], text: str) -> bool:
    """Some value of ``shape`` contains ``text``: ``text`` sits inside one
    literal piece, or starts in a piece's suffix, spans the pieces between
    (placeholders match anything) and ends in a piece's prefix."""
    whole = lambda p: ".*" if p is None else re.escape(p)
    suffix = lambda p: ".*" if p is None else "(?:" + "|".join(re.escape(p[i:]) for i in range(len(p))) + ")"
    prefix = lambda p: ".*" if p is None else "(?:" + "|".join(re.escape(p[:i]) for i in range(1, len(p) + 1)) + ")"
    for s, first in enumerate(shape):
        if first is not None and text in first:
            return True
        for e in range(s + 1, len(shape)):
            if re.fullmatch(suffix(first) + "".join(whole(p) for p in shape[s + 1:e]) + prefix(shape[e]), text, re.S):
                return True
    return False


def _worded_runs(pattern: str) -> list[str]:
    """The literal runs of a ``match=`` regex that contain a space."""
    runs = [re.sub(r"\\(.)", r"\1", run) for run in re.split(r"\\[dwsDWSb]|(?<!\\)[.^$*+?()\[\]{}|]", pattern)]
    return [run for run in runs if " " in run.strip()]


def _reworded(shape: list[str | None], text: str, others: list[str]) -> bool:
    """A literal piece holds ``text``'s words in order with one or two words
    inserted between them (``binary values in {0, 1}`` for ``binary in {0,
    1}``): a second raise site that rewords the message the test matches.
    A piece that holds another matched message (``Salt length must be at
    least`` beside ``Salt must be at least``) is that message, not a rewording."""
    words = text.split()
    loose = r"(?:\s+\S+){0,2}\s+".join(re.escape(w) for w in words)
    return any(p is not None and re.search(loose, p) and not any(t.strip() in p for t in [text, *others]) for p in shape)


def _raises_misses(fields: dict[str, Any], code: str) -> list[str]:
    """``ValueError 'binary in {0, 1}'`` is a ``pytest.raises(match=...)``
    the module must satisfy with its own wording. Checked only when the
    module raises that exception type with a string, and only for the
    worded fragments (literal runs between regex operators that contain a
    space): a one-word match such as ``'ghost'`` is usually the caller's own
    data interpolated, and a type the module never raises with a message is
    usually the caller's fake. 15 of 849 flagship-written prompts flag their
    gold (five families): a message the test's fake raises, or Enum's own
    ``is not a valid`` wording."""
    try:
        tree = ast.parse(code)
    except SyntaxError:
        return []
    raised: dict[str | None, list[list[str | None]]] = {}
    for n in ast.walk(tree):
        if isinstance(n, ast.Raise) and isinstance(n.exc, ast.Call) and n.exc.args and isinstance(n.exc.args[0], (ast.Constant, ast.JoinedStr)):
            name = getattr(n.exc.func, "attr", None) or getattr(n.exc.func, "id", None)
            raised.setdefault(name, []).extend(_string_shapes(n.exc.args[0]))
    shapes = _string_shapes(tree)
    misses = []
    worded = {e: _worded_runs(m.group(3)) for e in fields.get("raises", []) if (m := RAISES.match(e.strip()))}
    for entry, runs in worded.items():
        kind = RAISES.match(entry.strip()).group(1)
        if kind.split(".")[-1] not in raised:
            continue
        others = [t for e, rs in worded.items() if e != entry for t in rs]
        for text in runs:
            if not any(_produces(s, text) for s in shapes):
                misses.append(f"raises: no {kind} message contains {text!r}")
                break
            if any(_reworded(s, text, others) for s in raised[kind.split(".")[-1]]):
                misses.append(f"raises: a {kind} message rewords {text!r}; copy it verbatim")
                break
    return misses


SIGNATURE = re.compile(r"(?<![\w.])([A-Za-z_]\w*)\(([^()]*)\)")
LITERAL_KW = re.compile(r"(?:^|,)\s*([A-Za-z_]\w*)\s*=\s*(?:-?\d[\d_.]*|None|True|False|'[^']*'|\"[^\"]*\")\s*(?=,|$)")


def _signature_misses(fields: dict[str, Any], code: str) -> list[str]:
    """``Fetcher(total_timeout=5.0)`` in the intent is a default: the export's
    function or ``__init__`` — or a public method of an exported class —
    must give that parameter one. Only literal values count
    (``soft_delete(now=clock())`` is a call, not a default) and only bare
    names (``subprocess.run(check=True)`` is a call); silent when the
    function is undefined or takes ``**kwargs``. 5 of 849 flagship-written
    prompts flag their gold, each a default the gold leaves out or spells
    differently."""
    try:
        tree = ast.parse(code)
    except SyntaxError:
        return []
    signatures: dict[str, ast.arguments] = {}
    methods: dict[str, ast.arguments] = {}
    for n in tree.body:
        if isinstance(n, (ast.FunctionDef, ast.AsyncFunctionDef)):
            signatures[n.name] = n.args
        elif isinstance(n, ast.ClassDef):
            init = next((b for b in n.body if isinstance(b, ast.FunctionDef) and b.name == "__init__"), None)
            if init:
                signatures[n.name] = init.args
            if n.name in fields.get("exports", []):
                methods.update({b.name: b.args for b in n.body if isinstance(b, (ast.FunctionDef, ast.AsyncFunctionDef)) and not b.name.startswith("_")})
    misses = []
    for name, body in SIGNATURE.findall(str(fields.get("intent", ""))):
        a = signatures.get(name) if name in fields.get("exports", []) else methods.get(name)
        if a is None or a.kwarg:
            continue
        positional = a.posonlyargs + a.args
        defaulted = {p.arg for p in positional[len(positional) - len(a.defaults):]} if a.defaults else set()
        defaulted |= {p.arg for p, d in zip(a.kwonlyargs, a.kw_defaults) if d is not None}
        misses += [f"intent: {name}({k}=...) needs a default for {k}" for k in LITERAL_KW.findall(body) if k not in defaulted]
    return misses


FIX_CALL = re.compile(r"(?<![\w.'\"])((?:[A-Za-z_]\w*)?(?:\.[A-Za-z_]\w*)*)\(")
FIX_KW = re.compile(r"(?<![\w.])([a-z_]\w*)=('[^']*'|\"[^\"]*\"|-?\d+(?:\.\d+)?|True|False|None)(?![\w.])")
FIX_NEG = re.compile(r"\b(?:never|no|without|avoid|don't|do not)\s+(?:call(?:ing)?\s+|use\s+|using\s+)?`?$")


def fix_misses(fixes: list[str], code: str) -> list[str]:
    """What a flagship's ``fix:`` lines name that the DSL visibly lacks: a
    call-shaped ``json.dumps(`` must be called or defined, a ``never
    .decode()`` must not be called, and a literal ``mode='before'`` must be a
    keyword, default, or assignment with that value. Only the fix lines, not
    the whole intent: there the same atoms describe runtime values and
    signatures (104 of 849 flagship-written prompts flag their gold), while
    a fix line names what the last module missed (e2e-010: the model dropped
    ``mode='before'``, ``json.dumps``, ``split`` and kept ``.decode()``
    despite the flagship's fix)."""
    try:
        tree = ast.parse(code)
    except SyntaxError:
        return []
    nodes = list(ast.walk(tree))
    called = {getattr(n.func, "attr", None) or getattr(n.func, "id", None) for n in nodes if isinstance(n, ast.Call)}
    named = called | {n.name for n in nodes if isinstance(n, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef))} \
        | {n.attr for n in nodes if isinstance(n, ast.Attribute)} | {n.id for n in nodes if isinstance(n, ast.Name)}
    values = {(k.arg, repr(k.value.value)) for n in nodes if isinstance(n, ast.Call) for k in n.keywords if isinstance(k.value, ast.Constant)}
    for n in nodes:
        if isinstance(n, (ast.FunctionDef, ast.AsyncFunctionDef)):
            pos = n.args.posonlyargs + n.args.args
            pairs = [*zip(pos[len(pos) - len(n.args.defaults):], n.args.defaults), *zip(n.args.kwonlyargs, n.args.kw_defaults)]
            values |= {(p.arg, repr(d.value)) for p, d in pairs if isinstance(d, ast.Constant)}
        elif isinstance(n, ast.Assign) and isinstance(n.value, ast.Constant):
            values |= {(t.id, repr(n.value.value)) for t in n.targets if isinstance(t, ast.Name)}
        elif isinstance(n, ast.AnnAssign) and isinstance(n.value, ast.Constant) and isinstance(n.target, ast.Name):
            values.add((n.target.id, repr(n.value.value)))
    misses = []
    for fix in fixes:
        for m in FIX_CALL.finditer(fix):
            name = m.group(1).split(".")[-1]
            if not name:
                continue
            if FIX_NEG.search(fix[: m.start()]):
                if name in called:
                    misses.append(f"fix: {fix!r} but the code still calls {m.group(1)}()")
            elif name not in named:
                misses.append(f"fix: {fix!r} but the code never calls {m.group(1)}()")
        misses += [f"fix: {fix!r} but the code never passes {k}={v}" for k, v in FIX_KW.findall(fix)
                   if (k, repr(ast.literal_eval(v))) not in values]
    return misses


def _keyword_misses(fields: dict[str, Any], code: str) -> list[str]:
    """Each ``kwargs`` name is read from the patched callee's keywords, so a
    call to that callee must pass it by keyword (``fromstring(d, parser=p)``,
    not ``fromstring(d, p)``). Silent when the module never calls the callee
    directly or splats ``**kw`` into it."""
    callees = {p.split(".")[-1] for p in fields.get("patch", [])}
    try:
        tree = ast.parse(code)
    except SyntaxError:
        return []
    calls = [c for c in ast.walk(tree) if isinstance(c, ast.Call)
             and (getattr(c.func, "attr", None) or getattr(c.func, "id", None)) in callees]
    if not calls or any(k.arg is None for c in calls for k in c.keywords):
        return []
    passed = {k.arg for c in calls for k in c.keywords}
    return [f"kwargs: {k} is never passed by keyword to {', '.join(sorted(callees))}"
            for k in fields.get("kwargs", []) if k not in passed]


def _misses(fields: dict[str, Any], fixes: list[str], code: str) -> list[str]:
    """Unmet contract and fix points, or none when ``code`` does not parse."""
    try:
        ast.parse(code)
    except SyntaxError:
        return []
    return conformance(fields, code) + fix_misses(list(fixes), code)


def rejection(fields: dict[str, Any], code: str, fixes: list[str] = ()) -> tuple[str, list[str], list[str]]:
    """One attempt -> (module, ``compile_dsl`` diagnostics, unmet prompt and fix
    points): the words a retry turn carries, shared with ``dsl_pairs.retry_pairs``
    so training shows the model the same rejections ``generate_module`` sends."""
    doc = str(fields["intent"]).split(". ")[0].rstrip(".") + "."
    source, diags = compile_dsl(code, doc, list(fields.get("exports") or []))
    return source, diags, _misses(fields, list(fixes), code)


def generate_module(
    model, tokenizer, prompt: str, retries: int = RETRIES, fixes: list[str] = (), previous: str | None = None
) -> dict[str, Any]:
    """Prompt -> {"source", "diagnostics", "attempts": [{"dsl", "diagnostics"}]}.

    An attempt is retried when ``compile_dsl`` rejects it or ``conformance``
    or ``fix_misses`` (over the ``fix:`` points merged into ``prompt``) finds
    the prompt unmet. ``source`` is the first fully clean module, else
    the compiled one with the fewest conformance misses; it is empty only when
    no attempt compiled, and ``diagnostics`` then holds the last rejection.
    With ``previous`` (the DSL the caller is revising) and ``fixes``, the
    first attempt is a revision of it, not a fresh module. Each retry shows
    only the latest attempt (``revision_messages``). A malformed prompt
    raises ``PromptError``.
    """
    fields = parse_prompt(prompt)
    msgs = revision_messages(prompt, previous, [f"fix: {f}" for f in fixes]) if previous and fixes else dsl_messages(prompt)
    attempts: list[dict[str, Any]] = []
    diags: list[str] = []
    best: tuple[int, str, str, list[str]] | None = None
    for _ in range(retries):
        text = _complete(model, tokenizer, msgs)
        code = strip_fence(text)
        source, diags, misses = rejection(fields, code, fixes)  # misses ride along a rejection, so one retry can mend both
        attempts.append({"dsl": code, "diagnostics": diags + misses})
        if not diags and not misses:
            return {"source": source, "dsl": code, "diagnostics": [], "attempts": attempts}
        if not diags and (best is None or len(misses) < best[0]):
            best = (len(misses), source, code, misses)
        msgs = revision_messages(prompt, code, diags + misses)
    if best:
        return {"source": best[1], "dsl": best[2], "diagnostics": best[3], "attempts": attempts}
    return {"source": "", "dsl": "", "diagnostics": diags, "attempts": attempts}
