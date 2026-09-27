"""SOM MCP server: ``som_generate`` (SOM prompt -> module) and ``som_compile`` (DSL -> module).

``som_generate`` is the product path: a flagship model sends a SOM prompt
(``som_core.prompt``), the SOM model — ``$SOM_BASE`` plus the LoRA adapter at
``$SOM_ADAPTER``, loaded once per server — writes DSL, and ``compile_dsl``
finishes it, retrying on rejection (``som_core.generate``). ``som_compile``
is the older path where the flagship writes the DSL itself; ``som_core.dsl``
adds imports, layout, and the module docstring, and refuses a near-miss
before anything is written. ``som_generate`` returns the module it wrote, so the
flagship can judge it against the requirement and revise the prompt. Either
way the module lands at ``$SOM_OUT/candidate.py`` (default: the current directory) and every call is
appended to ``$SOM_OUT/calls.jsonl``. Run with ``python -m som_core.mcp_server``.
"""

from __future__ import annotations

import json
import os
from pathlib import Path

from mcp.server.mcpserver import MCPServer

from som_core.dsl import compile_dsl
from som_core.prompt import SPEC, PromptError, expand_contract, fix_points, revise

server = MCPServer("som")
_MODEL: tuple = ()
_LAST: list[str] = []  # the last full prompt, which a `fix:` revision joins
_FIXES: list[str] = []  # the fix points merged into it, checked by `fix_misses`
_DSL: list[str] = []  # the DSL behind the last candidate, which a `fix:` revision revises


def _out() -> Path:
    root = Path(os.environ.get("SOM_OUT", "."))
    root.mkdir(parents=True, exist_ok=True)
    return root


def _log(record: dict) -> None:
    with (_out() / "calls.jsonl").open("a") as log:
        log.write(json.dumps(record) + "\n")


@server.tool(description=SPEC + " The SOM model writes the module from it; returns the module (with any prompt points it left unmet) or why it failed. Revise only if the module contradicts the requirement.")
def som_generate(prompt: str) -> str:
    global _MODEL
    from som_core.generate import generate_module, load_model

    if not _MODEL:
        _MODEL = load_model(os.environ["SOM_BASE"], os.environ.get("SOM_ADAPTER") or None)
    try:
        points = fix_points(prompt)
        prompt = revise(_LAST[-1] if _LAST else None, expand_contract(prompt, Path.cwd()))
        fixes = _FIXES + points if points else []
        result = generate_module(*_MODEL, prompt, fixes=fixes, previous=_DSL[-1] if points and _DSL else None)
        _LAST[:] = [prompt]
        _FIXES[:] = fixes
        _DSL[:] = [result["dsl"]] if result["dsl"] else []
    except PromptError as exc:
        _log({"tool": "som_generate", "prompt": prompt, "error": str(exc)})
        return f"bad prompt: {exc}"
    _log({"tool": "som_generate", "prompt": prompt, "attempts": result["attempts"]})
    if not result["source"]:
        return f"failed after {len(result['attempts'])} attempts:\n" + "\n".join(result["diagnostics"])
    (_out() / "candidate.py").write_text(result["source"])
    unmet = "".join(f"unmet: {d}\n" for d in result["diagnostics"])
    return f"ok\n{unmet}```python\n{result['source']}```"


@server.tool()
def som_compile(code: str, exports: list[str], doc: str = "") -> str:
    """Compile terse Python to candidate.py. Omit imports (auto-added), docstring (pass `doc`), and formatting; one-line `if c: raise E()` is fine. `exports`: every name callers import from the module, including library names they import from it (re-exported for you). Security/correctness near-misses are rejected with a reason. Returns `ok` or the errors to fix."""
    source, diags = compile_dsl(code, doc, exports)
    _log({"tool": "som_compile", "code": code, "exports": exports, "doc": doc, "diagnostics": diags})
    if diags:
        return "rejected:\n" + "\n".join(diags)
    (_out() / "candidate.py").write_text(source)
    return f"ok ({source.count(chr(10))} lines)"


if __name__ == "__main__":
    server.run()
