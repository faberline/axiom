"""Fixture pass rate: generate every held-out family from its caption and run its oracle.

Rules this module owns:

- The held-out families are exactly the ``families.val`` list of the
  adapter's ``training_summary.json``, and the base model is the one that
  summary names; a summary whose ``lock_sha256`` differs from the current
  ``sources.lock.json`` is refused, so an adapter is never judged on another
  backbone.
- Each family runs the chain from its caption alone: L1, then L2, then L3,
  each prompt built exactly as training built it
  (:func:`som_core.train.layer_prompt` under the summary's ``conditioning``,
  wrapped by :func:`som_core.train.messages`) from the generated upstream
  records. The L3 text is read in the summary's ``l3_format``
  (:func:`som_core.wire.load_ops` for ``fenced``, a JSON array for ``json``,
  the default for a summary that predates the field).
- Under the summary's ``l3_mode`` ``blockwise`` (``whole`` when absent), L3
  runs one call per block of the topology in hand, in topology order, each
  prompt built by :func:`som_core.train.block_prompt` from the block
  operations generated before it and read by
  :func:`som_core.wire.load_block` for the ``NEXT`` block; a reply it
  refuses, or one cut off at the token cap, stops the family at stage
  ``l3``. One
  ``l3i`` call (:func:`som_core.train.imports_prompt`) then writes the
  header operations, and a reply holding anything but ``CREATE_FILE`` and
  ``ADD_IMPORT``, or cut off at the cap, stops it at stage ``l3i``. The
  retired ``blockwise`` mode is refused. The operation list assembled is
  the header operations followed by the block operations. Nothing from the family's stored records reaches the chain's
  prompts.
- ``teacher_forced`` also runs each layer from the family's gold upstream
  records: L2 from the gold plan (stage ``l2`` or ``ok``), and L3 from the
  gold plan and topology through assembly and the fixture, so the L3 fixture
  rate is the ceiling the chain can reach with perfect upstream layers.
- Decoding is greedy with :data:`REPETITION_PENALTY` over the last
  :data:`REPETITION_CONTEXT` tokens, and no reply may repeat a run of
  :data:`NO_REPEAT_NGRAM` tokens it has already written
  (:func:`no_repeat_ngram`): a looped line is longer than the penalty's
  window, which is where block replies ran to the cap. No committed gold
  reply repeats a run that long (the longest repeat is 78 tokens, in
  ``70-selenium``), so the ban never blocks the target. All three are
  written to the summary. Decoding stops
  at the tokenizer's ``eos_token`` (the chat template's end-of-turn), which the
  locked Qwen snapshot leaves out of ``eos_token_ids``.
- A family stops at the first stage that fails and is recorded with that
  stage: ``l1``/``l2``/``l3``/``l3i`` (not JSON, or refused by ``records.py``),
  ``scope`` (the generated topology is ``SCOPE_TOO_LARGE``), ``assemble``
  (the assembler refuses the operation list), ``fixture`` (the fixture exits
  non-zero), or ``pass``. A family without a fixture is left out of the rate's
  denominator whatever stage it reached (``no_fixture`` when it assembles);
  every other family is in it, so a refusal counts as a failure.
- The fixture and the quality gate are the corpus project's own
  (``scripts/judge_candidate.py``, run through ``uv run --project``), so the
  generated file is judged exactly as the gold was.
"""

from __future__ import annotations

import json
import subprocess
from collections import Counter
from pathlib import Path
from typing import Any

from .assemble import DEFAULT_ISA, assemble, load_isa, write_files
from .dataset import find_default_corpora, load_corpus
from .paths import sha256, write_json
from .records import RecordError, ScopeTooLarge, validate_plan, validate_topology
from .train import (
    HEADER_OPS, L3_MODE, LAYERS, LOCK, ROOT, TrainError, block_prompt, fetch_locked, imports_prompt, layer_prompt, messages,
)
from .wire import load_block, load_ops

MAX_TOKENS = {"l1": 512, "l2": 1024, "l3": 4096, "l3_block": 1024, "l3i": 512}
REPETITION_PENALTY = 1.1
REPETITION_CONTEXT = 20
NO_REPEAT_NGRAM = 80


def no_repeat_ngram(n: int, prompt_len: int):
    """A logits processor that bans any token completing an ``n``-token run the
    reply (the tokens after the first ``prompt_len``) already contains."""
    import mlx.core as mx

    def processor(tokens, logits):
        reply = tokens[prompt_len:].tolist()
        if len(reply) < n:
            return logits
        tail = reply[len(reply) - n + 1 :]
        banned = {reply[i + n - 1] for i in range(len(reply) - n + 1) if reply[i : i + n - 1] == tail}
        if not banned:
            return logits
        mask = mx.zeros(logits.shape[-1], dtype=mx.bool_)
        mask[mx.array(sorted(banned))] = True
        return mx.where(mask, -mx.inf, logits)

    return processor


def _generate(model, tokenizer, layer: str, prompt: str, l3_format: str = "json", l3_mode: str = "whole") -> str:
    from mlx_lm import generate
    from mlx_lm.sample_utils import make_logits_processors

    chat = tokenizer.apply_chat_template(messages(layer, prompt, "", l3_format, l3_mode)[:-1], add_generation_prompt=True, return_dict=False)
    processors = make_logits_processors(repetition_penalty=REPETITION_PENALTY, repetition_context_size=REPETITION_CONTEXT)
    processors.append(no_repeat_ngram(NO_REPEAT_NGRAM, len(chat)))
    return generate(model, tokenizer, chat, max_tokens=_cap(layer, l3_mode), logits_processors=processors)


def _cap(layer: str, l3_mode: str) -> int:
    return MAX_TOKENS["l3_block" if layer == "l3" and l3_mode != "whole" else layer]


def _capped(tokenizer, text: str, cap: int) -> bool:
    """Whether ``text`` filled the token cap, i.e. decoding was cut off rather than ended."""
    return tokenizer is not None and len(tokenizer.encode(text)) >= cap


def judge(corpus: Path, family: str, candidate_dir: Path) -> dict[str, Any]:
    """Run the corpus project's judge on ``candidate_dir``; its verdict JSON."""
    project = corpus.parents[1]
    proc = subprocess.run(
        ["uv", "run", "--quiet", "--project", str(project), "python", str(project / "scripts" / "judge_candidate.py"),
         "--corpus", str(corpus), family, str(candidate_dir)],
        capture_output=True, text=True, check=False,
    )
    if proc.returncode != 0:
        raise TrainError(f"judge_candidate.py exited {proc.returncode} on {family}: {proc.stderr.strip()}")
    return json.loads(proc.stdout)


def has_fixture(corpus: Path, family: str) -> bool:
    """The corpus harness's lookup: ``test_<family>.py``, underscored, or ``test_<number>_*.py``."""
    fixtures = corpus / "fixtures"
    names = (f"test_{family}.py", f"test_{family.replace('-', '_')}.py")
    return any((fixtures / n).is_file() for n in names) or any(fixtures.glob(f"test_{family.split('-')[0]}_*.py"))


def run_chain(
    model,
    tokenizer,
    caption: str,
    isa: dict[str, dict[str, Any]],
    out: Path,
    conditioning: str = "cumulative",
    given: dict[str, Any] | None = None,
    until: str = "l3",
    l3_format: str = "json",
    l3_mode: str = "whole",
) -> dict[str, Any]:
    """Generate each layer not in ``given`` up to ``until``, then assemble into ``out``.

    ``given`` maps a layer to its gold record, used instead of generating it.
    Returns the stage reached (``ok`` when ``until`` is not ``l3`` and every
    generated layer validated) and the raw generated text per layer.
    """
    record: dict[str, Any] = {}
    parsed: dict[str, Any] = dict(given or {})
    for layer in LAYERS[: LAYERS.index(until) + 1]:
        if layer in parsed:
            continue
        if layer == "l3" and l3_mode != "whole":
            failed = _blockwise_l3(model, tokenizer, caption, parsed, record)
            if failed:
                return failed
            continue
        prompt = layer_prompt(layer, caption, parsed.get("l1"), parsed.get("l2"), conditioning)
        text = _generate(model, tokenizer, layer, prompt, l3_format, l3_mode)
        record[layer] = text
        try:
            parsed[layer] = load_ops(text) if layer == "l3" and l3_format == "fenced" else json.loads(text)
            if layer == "l1":
                validate_plan(parsed[layer])
            elif layer == "l2":
                validate_topology(parsed[layer])
        except ScopeTooLarge as exc:
            return {**record, "stage": "scope", "error": str(exc)}
        except (json.JSONDecodeError, RecordError) as exc:
            return {**record, "stage": layer, "error": str(exc)}
    if until != "l3":
        return {**record, "stage": "ok"}
    try:
        files = assemble(parsed["l3"], parsed["l2"], isa)
    except (RecordError, KeyError, TypeError) as exc:
        return {**record, "stage": "assemble", "error": str(exc)}
    write_files(files, out)
    if "candidate.py" not in files:
        return {**record, "stage": "assemble", "error": "operation list creates no candidate.py"}
    return {**record, "stage": "assembled"}


def _blockwise_l3(model, tokenizer, caption: str, parsed: dict[str, Any], record: dict[str, Any]) -> dict[str, Any] | None:
    """Fill ``parsed["l3"]`` block by block, then header; return the failing result, or ``None``."""
    plan, topology = parsed["l1"], parsed["l2"]
    texts: list[str] = []
    blocks: list[dict[str, Any]] = []
    for file in topology["files"]:
        for spec in file["blocks"]:
            where = f"{file['path']}:{spec['name']}"
            text = _generate(model, tokenizer, "l3", block_prompt(caption, plan, topology, blocks, file["path"], spec["name"]),
                             "fenced", L3_MODE)
            texts.append(text)
            record["l3"] = "\n".join(texts)
            if _capped(tokenizer, text, _cap("l3", L3_MODE)):
                return {**record, "stage": "l3", "error": f"{where}: reply hit the {_cap('l3', L3_MODE)}-token cap"}
            try:
                blocks.append(load_block(text, file["path"], spec["name"]))
            except RecordError as exc:
                return {**record, "stage": "l3", "error": f"{where}: {exc}"}
    text = _generate(model, tokenizer, "l3i", imports_prompt(caption, plan, topology, blocks), "fenced", L3_MODE)
    record["l3i"] = text
    if _capped(tokenizer, text, _cap("l3i", L3_MODE)):
        return {**record, "stage": "l3i", "error": f"reply hit the {_cap('l3i', L3_MODE)}-token cap"}
    try:
        header = load_ops(text)
    except RecordError as exc:
        return {**record, "stage": "l3i", "error": str(exc)}
    if not header or any(o.get("op") not in HEADER_OPS for o in header):
        return {**record, "stage": "l3i", "error": f"expected CREATE_FILE/ADD_IMPORT only, got {[o.get('op') for o in header]}"}
    parsed["l3"] = header + blocks
    return None


def _judged(result: dict[str, Any], corpus: Path, family: str, target: Path) -> dict[str, Any]:
    result["has_fixture"] = has_fixture(corpus, family)
    if result["stage"] == "assembled":
        verdict = judge(corpus, family, target)
        result["quality"] = verdict["quality"]
        result["fixture_tail"] = verdict["fixture_tail"]
        result["stage"] = "no_fixture" if verdict["fixture"] is None else ("pass" if verdict["fixture"] == 0 else "fixture")
    return result


def _rate(results: list[dict[str, Any]]) -> dict[str, Any]:
    stages = Counter(r["stage"] for r in results)
    judged = [r for r in results if r["has_fixture"]]
    return {
        "stages": dict(sorted(stages.items())),
        "fixture_pass_rate": stages["pass"] / len(judged) if judged else None,
        "fixture_passed": stages["pass"],
        "fixture_denominator": len(judged),
        "quality_clean": sum(1 for r in results if "quality" in r and not r["quality"]),
    }


def evaluate(
    adapter: Path, out: Path, isa_dir: Path = DEFAULT_ISA, limit: int | None = None, teacher_forced: bool = False, log=print
) -> dict[str, Any]:
    from mlx_lm import load

    summary_path = adapter / "training_summary.json"
    if not summary_path.is_file():
        raise TrainError(f"no {summary_path}")
    summary = json.loads(summary_path.read_text())
    lock_path = ROOT / summary.get("lock", LOCK.name)
    if summary.get("lock_sha256") != sha256(lock_path):
        raise TrainError(f"{summary_path} was trained against another {lock_path.name}")
    conditioning = summary.get("conditioning", "previous")
    l3_format = summary.get("l3_format", "json")
    l3_mode = summary.get("l3_mode", "whole")
    if l3_mode not in ("whole", L3_MODE):
        raise TrainError(f"{summary_path} uses the retired l3_mode {l3_mode!r}; retrain it")
    model_dir, _ = fetch_locked(lock_path)
    model, tokenizer = load(str(model_dir), adapter_path=str(adapter))
    tokenizer.add_eos_token(tokenizer.eos_token)
    isa = load_isa(isa_dir)

    rows: dict[str, tuple[dict[str, Any], Path]] = {}
    for corpus in find_default_corpora():
        for row in load_corpus(corpus):
            rows[row["id"]] = (row, corpus)
    held_out = summary["families"]["val"][:limit] if limit else summary["families"]["val"]

    chain, forced_l2, forced_l3 = [], [], []
    for n, row_id in enumerate(held_out, 1):
        row, corpus = rows[row_id]
        family, meta = row["family"], row["metadata"]
        caption = meta["caption"]
        result = _judged(run_chain(model, tokenizer, caption, isa, out / "chain" / family, conditioning, l3_format=l3_format, l3_mode=l3_mode), corpus, family, out / "chain" / family)
        chain.append({"family": family, **result})
        line = f"[{n}/{len(held_out)}] {family}: chain {result['stage']}"
        if teacher_forced:
            plan, topology = meta["plan"], meta["decompiled"]["topology"]
            l2 = run_chain(model, tokenizer, caption, isa, out / "l2" / family, conditioning, {"l1": plan}, until="l2", l3_format=l3_format, l3_mode=l3_mode)
            forced_l2.append({"family": family, **l2})
            target = out / "l3" / family
            l3 = _judged(run_chain(model, tokenizer, caption, isa, target, conditioning, {"l1": plan, "l2": topology}, l3_format=l3_format, l3_mode=l3_mode), corpus, family, target)
            forced_l3.append({"family": family, **l3})
            line += f"  l2|gold {l2['stage']}  l3|gold {l3['stage']}"
        log(line)

    report: dict[str, Any] = {
        "adapter": str(adapter),
        "model": summary["model"],
        "revision": summary["revision"],
        "conditioning": conditioning,
        "l3_format": l3_format,
        "l3_mode": l3_mode,
        "decoding": {"greedy": True, "repetition_penalty": REPETITION_PENALTY, "repetition_context": REPETITION_CONTEXT, "no_repeat_ngram": NO_REPEAT_NGRAM,
                     "max_tokens": MAX_TOKENS},
        "families": len(chain),
        "chain": {**_rate(chain), "results": chain},
    }
    if teacher_forced:
        report["teacher_forced"] = {
            "l2": {"stages": dict(sorted(Counter(r["stage"] for r in forced_l2).items())), "results": forced_l2},
            "l3": {**_rate(forced_l3), "results": forced_l3},
        }
    write_json(out / "eval_summary.json", report)
    return report
