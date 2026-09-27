"""Training pairs for the SOM model: SOM prompt -> DSL.

The completion is always ``to_dsl(gold)``, which ``compile_dsl`` turns back
into the gold module (the round-trip gate in ``tests/test_dsl.py``). The
prompts come from three sources, one family at a time:

* mechanical: the caption as ``intent``, the fixture's ``contract_fields``,
  and each near miss's ``why_wrong`` as ``avoid``;
* flagship: SOM prompts a flagship model wrote from the caption and contract
  (``prompts_dir/<family>.json``, ``{"prompts": [...]}``), so the training
  distribution matches what ``som_generate`` receives; a contract tag the
  prompt lacks (``calls`` postdates the first recaption) is filled from the
  fixture, as the flagship copies it from the contract it is shown;
* condition dropout: ``dropout_copies`` copies of each prompt with optional
  tags removed, each copy its own draw, so an incomplete prompt still yields a
  whole module; the first copy is the same draw at every count;
* revision (``fix_pairs``): each near miss as a rejected attempt, its
  ``why_wrong`` as the ``fix:`` feedback, the gold as the reply, in the
  retry turn ``generate_module`` uses (``revision_messages``), so the model
  learns to change the code a fix names instead of re-emitting it;
* retry (``retry_pairs``): the same turn in the words ``generate_module``
  itself sends back (``generate.rejection``), from each near miss the loop
  would reject and from the gold with one worded ``raises`` message
  reworded, one guarded ``raise`` dropped, or one intent default knocked
  out. ``fix_pairs`` alone never shows those words, and som-dsl-007
  re-emitted the rejected attempt byte for byte on 8 of 16 retries in
  e2e-012;
* swap (``swap_pairs``): each prompt and the gold with the words of a worded
  ``raises`` message swapped alike, so the message is copied from the prompt
  rather than recalled from a similar family;
* fix retry (``fix_retry_pairs``): the retry after a ``fix:`` revision that
  still lacks the call or keyword the fix names, in the loop's own
  ``fix: '...' but the code never calls ...`` words.

Nothing here reads a holdout family; ``train`` removes those rows first.
"""

from __future__ import annotations

import ast
import io
import json
import random
import re
import tokenize
from pathlib import Path
from typing import Any

from som_core.dsl import to_dsl
from som_core.generate import DEFAULT, RAISES, _worded_runs, rejection
from som_core.prompt import contract_fields, parse_prompt, render_prompt

DROPPABLE = ("attrs", "keywords", "keys", "values", "avoid", "asserts")
INTERFACE = ("calls", "patch", "kwargs", "asserts")


def fixture_path(corpus: Path, family: str) -> Path:
    return corpus / "fixtures" / f"test_{family.replace('-', '_')}.py"


def gold_text(row: dict[str, Any]) -> str:
    return next(c["text"] for c in row["candidates"] if c["id"] == row["gold_candidate_id"])


def mechanical_prompt(row: dict[str, Any], corpus: Path) -> str:
    fx = fixture_path(corpus, row["family"])
    fields: dict[str, Any] = {"intent": row["metadata"]["caption"], **(contract_fields(fx.read_text()) if fx.is_file() else {})}
    avoid = [c["why_wrong"].strip().rstrip(".") for c in row["candidates"] if c.get("why_wrong")]
    if avoid:
        fields["avoid"] = avoid
    return render_prompt(fields)


def flagship_prompts(row: dict[str, Any], prompts_dir: Path | None, corpus: Path | None = None) -> list[str]:
    path = prompts_dir / f"{row['family']}.json" if prompts_dir else None
    prompts = list(json.loads(path.read_text())["prompts"]) if path and path.is_file() else []
    fx = fixture_path(corpus, row["family"]) if corpus else None
    if not prompts or not fx or not fx.is_file():
        return prompts
    contract = contract_fields(fx.read_text())
    filled = []
    for prompt in prompts:
        fields = parse_prompt(prompt)
        filled.append(render_prompt({**{t: contract[t] for t in INTERFACE if t in contract and t not in fields}, **fields}))
    return filled


def drop_conditions(prompt: str, rng: random.Random) -> str:
    fields = parse_prompt(prompt)
    present = [t for t in DROPPABLE if t in fields]
    for tag in rng.sample(present, k=rng.randint(1, len(present))) if present else []:
        del fields[tag]
    return render_prompt(fields)


def dsl_pairs(
    row: dict[str, Any], corpus: Path, prompts_dir: Path | None = None, dropout: bool = True, seed: int = 42,
    dropout_copies: int = 1,
) -> list[tuple[str, str, str]]:
    """Row -> [("dsl", prompt, completion)], one per distinct prompt."""
    code, _ = to_dsl(gold_text(row))
    prompts = [mechanical_prompt(row, corpus), *flagship_prompts(row, prompts_dir, corpus)]
    if dropout:
        rng = random.Random(f"{seed}:{row['id']}")
        whole = list(prompts)
        for _ in range(dropout_copies):
            prompts += [drop_conditions(p, rng) for p in whole]
    return [("dsl", p, code) for p in dict.fromkeys(prompts)]


def fix_pairs(row: dict[str, Any], corpus: Path, prompts_dir: Path | None = None) -> list[tuple[str, str, str]]:
    """Row -> [("dsl_fix", json [prompt, miss DSL, feedback], gold DSL)], one per near miss that parses."""
    code, _ = to_dsl(gold_text(row))
    prompt = (flagship_prompts(row, prompts_dir, corpus) or [mechanical_prompt(row, corpus)])[0]
    out = []
    for c in row["candidates"]:
        if not c.get("why_wrong"):
            continue
        try:
            miss, _ = to_dsl(c["text"])
        except SyntaxError:
            continue
        if miss != code:
            out.append(("dsl_fix", json.dumps([prompt, miss, [f"fix: {c['why_wrong'].strip()}"]]), code))
    return out


def _dropped_raises(code: str, texts: list[str]) -> list[str]:
    """The gold without one guarded ``raise`` whose message carries a worded
    fragment: the ``if`` it is the whole body of, or the bare statement when
    its block keeps others. Lines are cut from the text, so the rest of the
    module stays byte-identical and the retry target differs by that branch."""
    tree = ast.parse(code)
    parent = {child: node for node in ast.walk(tree) for child in ast.iter_child_nodes(node)}
    lines, out = code.splitlines(keepends=True), []
    for node in ast.walk(tree):
        if not isinstance(node, ast.Raise) or not any(t in (ast.get_source_segment(code, node) or "") for t in texts):
            continue
        up = parent[node]
        if isinstance(up, ast.If) and up.body == [node] and not up.orelse:
            cut = up
        elif len(getattr(up, "body", [])) > 1 and node in up.body:
            cut = node
        else:
            continue
        dropped = "".join(lines[: cut.lineno - 1] + lines[cut.end_lineno :])
        try:
            ast.parse(dropped)
        except SyntaxError:
            continue
        out.append(dropped)
    return out


def _knockouts(fields: dict[str, Any], code: str) -> list[str]:
    """The gold with one contract point broken the way an attempt breaks it:
    a worded ``raises`` fragment reworded by one inserted word (``binary
    value in {0, 1}``) or its whole guarded ``raise`` dropped
    (``_dropped_raises``; som-dsl-008 learned only the one-word repair and
    re-sent every e2e-013 attempt that lacked the branch), or an intent
    default written doubled."""
    out = []
    for entry in fields.get("raises", []):
        m = RAISES.match(entry.strip())
        texts = [t.strip() for t in _worded_runs(m.group(3))] if m else []
        for text in texts:
            first, _, rest = text.partition(" ")
            if text in code:
                out.append(code.replace(text, f"{first} value {rest}"))
        out += _dropped_raises(code, [t for t in texts if t in code])
    for num in DEFAULT.findall(str(fields.get("intent", ""))):
        literal = re.sub(r"[_,]", "", num)
        if re.fullmatch(r"-?\d+(\.\d+)?", literal):
            doubled = repr(float(literal) * 2) if "." in literal else str(int(literal) * 2 or 1)
            out.append(re.sub(rf"(?<![\w.]){re.escape(literal)}(?![\w.])", doubled, code))
    return [k for k in dict.fromkeys(out) if k != code]


# Plain error-message words a swap draws from; fixed here so no other row,
# and never a holdout row, leaks its wording into this one.
SWAP_WORDS = (
    "account", "archive", "batch", "bucket", "buffer", "cache", "channel", "chunk", "column", "cursor",
    "digest", "domain", "draft", "entry", "folder", "frame", "handle", "header", "index", "ledger",
    "limit", "marker", "member", "module", "offset", "order", "owner", "packet", "parcel", "policy",
    "profile", "quota", "record", "region", "report", "sample", "schema", "segment", "sensor", "shard",
    "signal", "socket", "source", "stage", "status", "stream", "tenant", "ticket", "timer", "vector",
    "blank", "broken", "closed", "empty", "expired", "frozen", "hidden", "locked", "missing", "stale",
)
KEEP_WORDS = frozenset(
    "the and for not must can cannot should only than that this with from into are was were has have got "
    "does did any all per least most more less one two zero none true false between within after before "
    "under over below above without".split()
)


def _in_strings(code: str, text: str) -> bool:
    """Every occurrence of ``text`` in ``code`` lies inside a string token."""
    tokens = tokenize.generate_tokens(io.StringIO(code).readline)
    inside = sum(t.string.count(text) for t in tokens if t.type in (tokenize.STRING, getattr(tokenize, "FSTRING_MIDDLE", -1)))
    return inside == code.count(text) > 0


def _swapped(span: str, words: dict[str, str]) -> str:
    return re.sub(r"[A-Za-z]+", lambda m: words.get(m.group(), m.group()), span)


def _swap_words(texts: list[str], rng: random.Random) -> dict[str, str]:
    """Each content word of the fragments -> an unused ``SWAP_WORDS`` word, case kept."""
    words: dict[str, str] = {}
    pool = [w for w in SWAP_WORDS if not any(w in t.lower() for t in texts)]
    rng.shuffle(pool)
    for word in dict.fromkeys(w for t in texts for w in re.findall(r"[A-Za-z]+", t)):
        if len(word) < 3 or word.lower() in KEEP_WORDS or not pool:
            continue
        new = pool.pop()
        words[word] = new.upper() if word.isupper() and len(word) > 1 else new.capitalize() if word[0].isupper() else new
    return words


def swap_pairs(
    row: dict[str, Any], corpus: Path, prompts_dir: Path | None = None, seed: int = 42
) -> list[tuple[str, str, str]]:
    """Row -> [("dsl_swap", prompt, DSL)]: each prompt with the content words of
    its worded ``raises`` fragments swapped for other words, and the gold with
    the same swap wherever the fragment sits in a string, so the only source of
    the message the model writes is the prompt. som-dsl-009 wrote 48's
    ``binary in {0, 1}`` as ``y_true must contain only 0 and 1``: a message
    recalled from similar families, not copied from the prompt. A variant is
    kept only when the loop would accept it (``rejection`` finds no diagnostic
    and no more unmet points than the gold's)."""
    code, _ = to_dsl(gold_text(row))
    mechanical = mechanical_prompt(row, corpus)
    fields = parse_prompt(mechanical)
    _, gold_diags, gold_misses = rejection(fields, code)
    if gold_diags:
        return []
    texts = []
    for entry in fields.get("raises", []):
        m = RAISES.match(entry.strip())
        texts += [t.strip() for t in _worded_runs(m.group(3))] if m else []
    texts = [t for t in dict.fromkeys(texts) if _in_strings(code, t)]
    if not texts:
        return []
    # a fragment shows up escaped in a prompt's regex (`\{0, 1\}`): match each
    # non-alphanumeric character with an optional backslash
    spans = [re.compile("".join(re.escape(c) if c.isalnum() else r"\\?" + re.escape(c) for c in t)) for t in texts]
    out = []
    for prompt in dict.fromkeys([mechanical, *flagship_prompts(row, prompts_dir, corpus)]):
        words = _swap_words(texts, random.Random(f"{seed}:{row['id']}:{prompt}"))
        if not words:
            continue
        new_prompt, new_code = prompt, code
        for text, span in zip(texts, spans):
            new_prompt = span.sub(lambda m: _swapped(m.group(), words), new_prompt)
            new_code = new_code.replace(text, _swapped(text, words))
        if new_prompt == prompt or new_code == code:
            continue
        try:
            new_fields = parse_prompt(new_prompt)
        except ValueError:
            continue
        _, diags, misses = rejection(new_fields, new_code)
        if not diags and len(misses) <= len(gold_misses):
            out.append(("dsl_swap", new_prompt, new_code))
    return out


def retry_pairs(row: dict[str, Any], corpus: Path) -> list[tuple[str, str, str]]:
    """Row -> [("dsl_retry", json [prompt, attempt, feedback], gold DSL)], one per
    attempt the loop would reject with feedback the gold clears: each distinct
    near miss, then each ``_knockouts`` variant that still compiles."""
    code, _ = to_dsl(gold_text(row))
    prompt = mechanical_prompt(row, corpus)
    fields = parse_prompt(prompt)
    _, gold_diags, gold_misses = rejection(fields, code)
    if gold_diags:
        return []
    attempts = []
    for c in row["candidates"]:
        if c.get("why_wrong"):
            try:
                attempts.append(to_dsl(c["text"])[0])
            except SyntaxError:
                continue
    attempts += _knockouts(fields, code)
    out = []
    for attempt in dict.fromkeys(a for a in attempts if a != code):
        _, diags, misses = rejection(fields, attempt)
        feedback = diags + [m for m in misses if m not in gold_misses]
        if feedback:
            out.append(("dsl_retry", json.dumps([prompt, attempt, feedback]), code))
    return out


# How a fix line names what the last module lacked; generic, so no family's
# (and never a holdout family's) wording rides along.
CALL_FIXES = ("call {c}()", "must call {c}()", "{c}() is never called; call it", "also call {c}()")
KW_FIXES = ("pass {k}={v} to {c}()", "{c}() needs {k}={v}", "set {k}={v}")


def _chain(func: ast.expr) -> str | None:
    parts = []
    while isinstance(func, ast.Attribute):
        parts.insert(0, func.attr)
        func = func.value
    return ".".join([func.id, *parts]) if isinstance(func, ast.Name) else None


def _fix_knockouts(code: str) -> list[tuple[str, str]]:
    """The gold with one thing a fix line can name cut out -> (that fix line's
    template filled in, the cut module): a statement that is a bare call
    (``pipe.reset()``; ``pass`` keeps its block), or one constant keyword of a
    call (``min_iterations=600000``). Text is cut, so the rest stays byte-identical."""
    tree = ast.parse(code)
    lines = code.splitlines(keepends=True)
    offsets = [0]
    for line in lines:
        offsets.append(offsets[-1] + len(line))
    out = []
    for node in ast.walk(tree):
        for block in (getattr(node, f, None) for f in ("body", "orelse", "finalbody")):
            for stmt in block if isinstance(block, list) else []:
                call = stmt.value if isinstance(stmt, ast.Expr) else None
                call = call.value if isinstance(call, ast.Await) else call
                if isinstance(call, ast.Call) and (c := _chain(call.func)):
                    keep = [lines[stmt.lineno - 1][: stmt.col_offset] + "pass\n"] if len(block) == 1 else []
                    out.append((CALL_FIXES, {"c": c}, "".join(lines[: stmt.lineno - 1] + keep + lines[stmt.end_lineno :])))
        if isinstance(node, ast.Call) and (c := _chain(node.func)):
            args = [*node.args, *node.keywords]
            for kw in node.keywords:
                if kw.arg and isinstance(kw.value, ast.Constant) and not isinstance(kw.value.value, bytes):
                    start = offsets[kw.lineno - 1] + kw.col_offset
                    end = offsets[kw.end_lineno - 1] + kw.end_col_offset
                    i = args.index(kw)
                    if i:  # from the end of the argument before it
                        prev = args[i - 1]
                        start = offsets[prev.end_lineno - 1] + prev.end_col_offset
                    elif i + 1 < len(args):  # to the start of the one after it
                        nxt = args[i + 1]
                        end = offsets[nxt.lineno - 1] + nxt.col_offset
                    v = ast.get_source_segment(code, kw.value) or ""
                    out.append((KW_FIXES, {"c": c, "k": kw.arg, "v": v}, code[:start] + code[end:]))
    return [(t, n, cut) for t, n, cut in out if _parses(cut)]


def _parses(code: str) -> bool:
    try:
        ast.parse(code)
    except SyntaxError:
        return False
    return True


def fix_retry_pairs(row: dict[str, Any], corpus: Path, per_family: int = 2, seed: int = 42) -> list[tuple[str, str, str]]:
    """Row -> [("dsl_fix_retry", json [prompt, attempt, feedback], gold DSL)]: the
    retry after a ``fix:`` revision that still lacks what the fix names. The
    prompt's intent carries the fix line, as ``som_generate`` merges it; the
    attempt is a ``_fix_knockouts`` cut; the feedback is ``rejection``'s own
    ``fix: '...' but the code never calls pipe.reset()``. No other pair shows
    those words, and 6 of 7 such retries in e2e-015 (01, 40, 72) re-sent the
    rejected module byte for byte, while first revision turns, which
    ``fix_pairs`` trains, changed on 26 of 27. Kept only when the gold clears
    every fix miss and the cut draws one."""
    code, _ = to_dsl(gold_text(row))
    base = parse_prompt(mechanical_prompt(row, corpus))
    rng = random.Random(f"{seed}:{row['id']}:fix-retry")
    cuts = _fix_knockouts(code)
    rng.shuffle(cuts)
    out = []
    for templates, names, attempt in cuts:
        if len(out) >= per_family:
            break
        line = rng.choice(templates).format(**names)
        fields = {**base, "intent": f"{base['intent']}; {line}"}
        _, gold_diags, gold_misses = rejection(fields, code, [line])
        if gold_diags or any(m.startswith("fix:") for m in gold_misses):
            continue
        _, diags, misses = rejection(fields, attempt, [line])
        feedback = diags + [m for m in misses if m not in gold_misses]
        if any(m.startswith("fix:") for m in feedback):
            out.append(("dsl_fix_retry", json.dumps([render_prompt(fields), attempt, feedback]), code))
    return out

