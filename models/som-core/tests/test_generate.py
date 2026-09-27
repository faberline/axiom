"""The retry loop's prompt check: defaults the intent states must be written."""

from __future__ import annotations

from pathlib import Path

from som_core.dataset import load_corpora
from som_core.dsl import to_dsl
from som_core.dsl_pairs import fixture_path, gold_text
from som_core import generate
from som_core.generate import conformance, fix_misses, generate_module, revision_messages
from som_core.prompt import contract_fields, parse_prompt, render_prompt

CORPUS = Path(__file__).resolve().parents[2] / "som-code-python" / "data" / "curated"


def test_a_stated_default_that_is_missing_is_reported():
    intent = {"intent": "size defaulting to 1,000,000 bytes, x defaults to 5.0, and n default 1024"}
    assert conformance(intent, "def f(size=1_000_000, x=5.0, n=1000): pass") == ["intent: default 1024 is never written"]


# raises matches the gold never writes: the test's own fake raises them, or Enum words them.
CALLER_RAISED = [
    "155-syntax-enum-missing-legacy-values",
    "177-mock-side-effect-scripted-retries",
    "260-playwright-goto-retry-backoff",
    "286-selenium-page-object-login",
    "69-playwright-context-lifecycle",
]


def test_no_gold_module_fails_its_own_mechanical_prompt():
    flagged = []
    for row in load_corpora():
        fx = fixture_path(CORPUS, row["family"])
        if fx.is_file():
            fields = parse_prompt(render_prompt({"intent": row["metadata"]["caption"], **contract_fields(fx.read_text())}))
            misses = conformance(fields, to_dsl(gold_text(row))[0])
            if misses and not (row["family"] in CALLER_RAISED and all(m.startswith("raises:") for m in misses)):
                flagged.append(row["family"])
    assert flagged == []


def test_a_kwarg_the_patched_callee_reads_must_be_passed_by_keyword():
    fields = {"patch": ["etree.fromstring"], "kwargs": ["parser"]}
    assert conformance(fields, "def f(d, p):\n    return etree.fromstring(d, p)\n") == [
        "kwargs: parser is never passed by keyword to fromstring"]
    assert conformance(fields, "def f(d, p):\n    return etree.fromstring(d, parser=p)\n") == []


def test_a_literal_keyword_in_the_intent_signature_must_be_a_default():
    fields = {"intent": "Fetcher(total=5.0, name='x'); soft_delete(now=clock())", "exports": ["Fetcher", "soft_delete"]}
    required = "class Fetcher:\n    def __init__(self, total, name='x'):\n        pass\ndef soft_delete(now):\n    pass\n"
    assert conformance(fields, required) == ["intent: Fetcher(total=...) needs a default for total"]
    assert conformance(fields, required.replace("self, total,", "self, total=5.0,")) == []


def test_a_method_of_an_exported_class_owes_its_literal_default_and_a_dotted_call_does_not():
    fields = {"intent": "collect(model='m', n=None) then subprocess.run(check=True)", "exports": ["H"]}
    code = "class H:\n    def collect(self, model, n=None):\n        pass\n    def run(self, check):\n        pass\n"
    assert conformance(fields, code) == ["intent: collect(model=...) needs a default for model"]


def test_a_worded_raises_message_the_module_rewords_is_reported():
    fields = {"raises": ["ValueError \"Sensitive claim 'password' is forbidden\"", "ValueError 'binary in {0, 1}'", "KeyError 'ghost'"]}
    reworded = "def f(k):\n    raise ValueError(f'Sensitive claim \"{k}\" is forbidden in payload')\n"
    assert conformance(fields, reworded) == [
        "raises: no ValueError message contains \"Sensitive claim 'password' is forbidden\"",
        "raises: no ValueError message contains 'binary in '"]
    kept = "def f(k):\n    raise ValueError(f\"Sensitive claim '{k}' is forbidden in payload\")\n    raise ValueError('labels must be binary in {0, 1}')\n"
    assert conformance(fields, kept) == []
    assert conformance(fields, "def f():\n    raise RuntimeError('x')\n") == []


def test_single_quoted_literals_reach_the_model_double_quoted_and_prose_does_not():
    from som_core.generate import dsl_messages

    user = dsl_messages("intent: PHONE r'^\\+\\d$'; don't strip users' names; 'a\"b'")[1]["content"]
    assert user == "intent: PHONE r\"^\\+\\d$\"; don't strip users' names; 'a\"b'"


def test_a_second_raise_site_that_rewords_the_matched_message_is_reported():
    fields = {"raises": ["ValueError 'binary in {0, 1}'", "ValueError 'Salt length must be at least'", "ValueError 'Salt must be at least'"]}
    code = ("def f(x):\n    raise ValueError('pos_label must be binary in {0, 1}')\n"
            "    raise ValueError('y_true must contain only binary values in {0, 1}')\n"
            "    raise ValueError('Salt must be at least 16')\n    raise ValueError('Salt length must be at least 16')\n")
    assert conformance(fields, code) == ["raises: a ValueError message rewords 'binary in '; copy it verbatim"]
    assert conformance(fields, code.replace("binary values in", "binary in")) == []
    docstring = code.replace("binary values in", "binary in").replace("(x):\n", '(x):\n    """only binary values in {0, 1}"""\n', 1)
    assert conformance(fields, docstring) == []  # only raise messages count


def test_a_fix_point_the_module_ignores_is_reported_and_a_met_one_is_not():
    code = "def f(m):\n    return json.dumps(m['data'].decode(), indent=2)\n\nclass V:\n    mode = 'after'\n"
    assert fix_misses(["never .decode()", "validate_phone mode='before'", "use os.replace(tmp, path)"], code) == [
        "fix: 'never .decode()' but the code still calls .decode()",
        "fix: \"validate_phone mode='before'\" but the code never passes mode='before'",
        "fix: 'use os.replace(tmp, path)' but the code never calls os.replace()",
    ]
    assert fix_misses(["json.dumps(indent=2) then f()", "no pipe.reset()"], code) == []


def test_a_fix_revises_the_previous_dsl_and_a_retry_shows_only_the_latest_attempt(monkeypatch):
    seen = []
    replies = iter(["def f(x):\n    return y\n", "def f(x):\n    return x\n"])

    def fake(model, tokenizer, msgs):
        seen.append(msgs)
        return next(replies)

    monkeypatch.setattr(generate, "_complete", fake)
    prompt = render_prompt({"intent": "f returns x.", "exports": ["f"]})
    out = generate_module(None, None, prompt, fixes=["return x"], previous="def f(x):\n    pass\n")
    assert out["dsl"] == "def f(x):\n    return x\n" and out["source"]
    assert seen[0][-2] == {"role": "assistant", "content": "def f(x):\n    pass\n"}
    assert seen[0][-1]["content"].startswith("rejected:\nfix: return x")
    assert seen[1][-2]["content"] == "def f(x):\n    return y\n" and len(seen[1]) == len(seen[0])


def test_revision_messages_append_the_attempt_the_feedback_and_the_completion():
    msgs = revision_messages(render_prompt({"intent": "f.", "exports": ["f"]}), "A", ["fix: b"], "C")
    assert [m["role"] for m in msgs[-3:]] == ["assistant", "user", "assistant"]
    assert msgs[-3]["content"] == "A" and "fix: b" in msgs[-2]["content"] and msgs[-1]["content"] == "C"
