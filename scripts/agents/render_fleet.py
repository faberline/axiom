#!/usr/bin/env python3
"""Render the per-project agent fleet and its Codex projection.

Sources of truth, in this order:

- ``scripts/agents/templates/<tier>/<role>.md`` — one template per tier
  (``app`` or ``lib``) and role, with ``{project}`` as the only placeholder.
- ``PROJECTS`` below — the explicit project list. Six projects carry no
  ``aw.toml``, so nothing here discovers projects from it.
- Hand-written singleton ``.claude/agents/<name>.md`` files (``aw-dev``,
  ``cto``, ``project-manager``, …). They are never rewritten; only their Codex
  projection is rendered.

Outputs:

- ``.claude/agents/<project>-<role>.md`` for every listed project and role.
- ``.codex/agents/<name>.toml`` for every ``.claude/agents/*.md`` — rendered
  and singleton alike — carrying ``name``, ``description``, the pinned
  ``model_reasoning_effort``, and the markdown body as
  ``developer_instructions``. A ``*.toml`` with no markdown twin is stray and
  is deleted on ``--write``.

``--check`` writes nothing and exits 1 on any byte difference, missing file,
or stray projection; ``--write`` makes the tree match. Runs on the system
``python3`` (3.9): no ``tomllib``, no ``match``.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Dict, Iterable, List, Tuple


APPS: Tuple[str, ...] = (
    "arena", "aw", "beam", "cap", "cgdb", "courier", "cube", "defer", "guard",
    "jet", "keep", "loom", "mamba", "mesh", "meter", "pgpool", "pm",
    "preview", "relay", "sift", "tape", "vat", "workbench",
)
# The shared libraries moved to faberline/core, so no lib-tier project
# remains here to render roles for.
LIBS: Tuple[str, ...] = ()
TIER_ROLES: Dict[str, Tuple[str, ...]] = {
    "app": ("pm", "tl", "qa", "dev"),
    "lib": ("pm", "tl", "qa", "dev"),
}
# apps/aw is a Python uv project whose implementation agent is the hand-written
# singleton `aw-dev`. It gets PM, TL, and QA roles from the templates; the
# custom Dev role keeps the Python/uv verification contract.
ROLE_OVERRIDES: Dict[str, Tuple[str, ...]] = {"aw": ("pm", "tl", "qa")}

PROJECTS: Tuple[Tuple[str, str], ...] = tuple(
    [("app", name) for name in APPS] + [("lib", name) for name in LIBS]
)

CODEX_DEFAULT_MODEL = "gpt-5.6-terra"
READ_ONLY_ROLES = ("cto", "integration-qa")
PLACEHOLDER = "{project}"

_FRONTMATTER_LINE = re.compile(r"^(name|description|effort):[ \t]*(.*?)[ \t]*$")


class RenderError(ValueError):
    """A source file cannot be rendered or projected."""


def repo_root() -> Path:
    return Path(__file__).resolve().parents[2]


def templates_dir(root: Path) -> Path:
    return root / "scripts" / "agents" / "templates"


def claude_agents_dir(root: Path) -> Path:
    return root / ".claude" / "agents"


def codex_agents_dir(root: Path) -> Path:
    return root / ".codex" / "agents"


def singleton_templates_dir(root: Path) -> Path:
    return root / "scripts" / "agents" / "singletons"


def project_roles(tier: str, name: str) -> Tuple[str, ...]:
    return ROLE_OVERRIDES.get(name, TIER_ROLES[tier])


def render_markdown(template: str, name: str) -> str:
    if PLACEHOLDER not in template:
        raise RenderError(f"template carries no {PLACEHOLDER} placeholder")
    return template.replace(PLACEHOLDER, name)


def rendered_agents(root: Path) -> Dict[str, str]:
    """Map agent name -> rendered markdown for every listed project and role."""
    out: Dict[str, str] = {}
    cache: Dict[Tuple[str, str], str] = {}
    for tier, name in PROJECTS:
        for role in project_roles(tier, name):
            key = (tier, role)
            if key not in cache:
                path = templates_dir(root) / tier / f"{role}.md"
                if not path.is_file():
                    raise RenderError(f"missing template: {path}")
                cache[key] = path.read_text(encoding="utf-8")
            agent = f"{name}-{role}"
            if agent in out:
                raise RenderError(f"duplicate rendered agent: {agent}")
            out[agent] = render_markdown(cache[key], name)
    return out


def split_frontmatter(text: str, source: str) -> Tuple[Dict[str, str], str]:
    lines = text.split("\n")
    if not lines or lines[0] != "---":
        raise RenderError(f"{source}: no YAML frontmatter")
    try:
        end = next(i for i, line in enumerate(lines[1:], start=1) if line == "---")
    except StopIteration:
        raise RenderError(f"{source}: unterminated YAML frontmatter") from None
    fields: Dict[str, str] = {}
    for line in lines[1:end]:
        match = _FRONTMATTER_LINE.match(line)
        if match:
            key, value = match.groups()
            if key in fields:
                raise RenderError(f"{source}: repeats {key}")
            fields[key] = value
    for key in ("name", "description", "effort"):
        if not fields.get(key):
            raise RenderError(f"{source}: frontmatter has no {key}")
    body = "\n".join(lines[end + 1:]).strip("\n")
    if not body:
        raise RenderError(f"{source}: empty body")
    if "'''" in body:
        raise RenderError(f"{source}: body contains ''' and cannot be projected")
    return fields, body


def toml_projection(markdown: str, source: str) -> str:
    fields, body = split_frontmatter(markdown, source)
    name = fields["name"]
    model = codex_model(name)
    return (
        f"name = {json.dumps(name)}\n"
        f"description = {json.dumps(fields['description'])}\n"
        f"model = {json.dumps(model)}\n"
        f"model_reasoning_effort = {json.dumps(fields['effort'])}\n"
        f"sandbox_mode = {json.dumps(codex_sandbox(name))}\n"
        f"nickname_candidates = [{json.dumps(name)}, "
        f"{json.dumps(name.replace('-', '_'))}]\n"
        "\n"
        "developer_instructions = '''\n"
        f"{body}\n"
        "'''\n"
    )


def codex_model(name: str) -> str:
    """Return the fixed Codex model for one registered fleet role."""
    if name == "integration-qa" or name == "cto":
        return "gpt-5.6-terra"
    if name.endswith("-pm") or name.endswith("-tl"):
        return "gpt-5.6-terra"
    if name.endswith("-qa") or name.endswith("-dev"):
        return "gpt-5.6-luna"
    return CODEX_DEFAULT_MODEL


def codex_sandbox(name: str) -> str:
    """Return the active prompt-only fleet's least broad role mode.

    The mode is a desktop sandbox default. It does not enforce the role's
    assigned project paths; those remain controller and prompt boundaries.
    """
    if name in READ_ONLY_ROLES or name.endswith("-pm") or name.endswith("-tl"):
        return "read-only"
    return "workspace-write"


def expected_files(root: Path) -> Dict[Path, str]:
    """Every path the renderer owns, mapped to its expected content."""
    rendered = rendered_agents(root)
    expected: Dict[Path, str] = {}
    markdown: Dict[str, str] = {}
    for agent, text in rendered.items():
        expected[claude_agents_dir(root) / f"{agent}.md"] = text
        markdown[agent] = text
    for path in sorted(claude_agents_dir(root).glob("*.md")):
        agent = path.stem
        if agent in markdown:
            continue
        markdown[agent] = path.read_text(encoding="utf-8")
    for agent, text in markdown.items():
        fields, _ = split_frontmatter(text, f"{agent}.md")
        if fields["name"] != agent:
            raise RenderError(f"{agent}.md: frontmatter name is {fields['name']!r}")
        expected[codex_agents_dir(root) / f"{agent}.toml"] = toml_projection(
            text, f"{agent}.md"
        )
    return expected


def expected_codex_files(root: Path) -> Dict[Path, str]:
    """Return active Codex role files without changing Claude's fleet."""
    markdown = rendered_agents(root)
    for path in sorted(claude_agents_dir(root).glob("*.md")):
        agent = path.stem
        if agent not in markdown:
            markdown[agent] = path.read_text(encoding="utf-8")
    for path in sorted(singleton_templates_dir(root).glob("*.md")):
        agent = path.stem
        if agent in markdown:
            raise RenderError(f"duplicate Codex singleton source: {agent}")
        markdown[agent] = path.read_text(encoding="utf-8")
    return {
        codex_agents_dir(root) / f"{agent}.toml": toml_projection(
            text, f"{agent}.md"
        )
        for agent, text in markdown.items()
    }


def stray_projections(root: Path, expected: Iterable[Path]) -> List[Path]:
    owned = set(expected)
    return [
        path
        for path in sorted(codex_agents_dir(root).glob("*.toml"))
        if path not in owned
    ]


def check(root: Path) -> List[str]:
    """Return one finding per divergence between the tree and the renderer."""
    expected = expected_files(root)
    findings: List[str] = []
    for path, content in sorted(expected.items()):
        rel = path.relative_to(root)
        if not path.is_file():
            findings.append(f"missing: {rel}")
        elif path.read_text(encoding="utf-8") != content:
            findings.append(f"differs: {rel}")
    for path in stray_projections(root, expected):
        findings.append(f"stray projection: {path.relative_to(root)}")
    return findings


def check_codex(root: Path) -> List[str]:
    """Return Codex-role divergences without treating Claude files as outputs."""
    findings: List[str] = []
    for path, content in sorted(expected_codex_files(root).items()):
        rel = path.relative_to(root)
        if not path.is_file():
            findings.append(f"missing: {rel}")
        elif path.read_text(encoding="utf-8") != content:
            findings.append(f"differs: {rel}")
    return findings


def write(root: Path) -> List[str]:
    """Make the tree match; return one line per path written or removed."""
    expected = expected_files(root)
    actions: List[str] = []
    for path, content in sorted(expected.items()):
        if path.is_file() and path.read_text(encoding="utf-8") == content:
            continue
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")
        actions.append(f"wrote: {path.relative_to(root)}")
    for path in stray_projections(root, expected):
        path.unlink()
        actions.append(f"removed: {path.relative_to(root)}")
    return actions


def write_codex(root: Path) -> List[str]:
    """Write active Codex roles only; preserve Claude and extra Codex files."""
    actions: List[str] = []
    for path, content in sorted(expected_codex_files(root).items()):
        if path.is_file() and path.read_text(encoding="utf-8") == content:
            continue
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")
        actions.append(f"wrote: {path.relative_to(root)}")
    return actions


def main(argv: Iterable[str] = ()) -> int:
    parser = argparse.ArgumentParser(
        description="Render the per-project agent fleet and its Codex projection."
    )
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--check", action="store_true",
                      help="report divergences and exit 1 on any; write nothing")
    mode.add_argument("--write", action="store_true",
                      help="write every owned file and remove stray projections")
    mode.add_argument("--check-codex", action="store_true",
                      help="report active Codex role divergences; write nothing")
    mode.add_argument("--write-codex", action="store_true",
                      help="write active Codex roles only; preserve Claude files")
    parser.add_argument("--root", type=Path, default=repo_root(),
                        help="repository root (default: this checkout)")
    args = parser.parse_args(list(argv) or None)
    root = args.root.resolve()
    try:
        if args.check:
            findings = check(root)
            for line in findings:
                print(line)
            if findings:
                print(f"render_fleet: {len(findings)} divergence(s); "
                      "run scripts/agents/render_fleet.py --write",
                      file=sys.stderr)
                return 1
            print("render_fleet: fleet matches its templates")
            return 0
        if args.check_codex:
            findings = check_codex(root)
            for line in findings:
                print(line)
            if findings:
                print(f"render_fleet: {len(findings)} Codex divergence(s); "
                      "run scripts/agents/render_fleet.py --write-codex",
                      file=sys.stderr)
                return 1
            print("render_fleet: Codex fleet matches its templates")
            return 0
        if args.write_codex:
            for line in write_codex(root):
                print(line)
            return 0
        for line in write(root):
            print(line)
        return 0
    except RenderError as exc:
        print(f"render_fleet: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
