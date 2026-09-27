#!/usr/bin/env python3
"""Validate one app or library README contract without changing it.

The deterministic pass checks only facts that can be decided from the current
checkout. The ``prompt`` command emits the separate clean-reader task used for
semantic comprehension review.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import shlex
import subprocess
import sys
import tomllib
from dataclasses import dataclass, field
from pathlib import Path


GIT = ("git", "-c", "core.fsmonitor=false")
REQUIRED_H2 = (
    "Brief",
    "Primary workflow",
    "Contract discovery",
    "Capabilities",
    "Supporting documents",
)
CAPABILITY_HEADER = ("Capability", "ID", "User promise", "Sources")
ID_RE = re.compile(r"^[a-z0-9]+(?:-[a-z0-9]+)*$")
SOURCE_RE = re.compile(
    r"`((?:apps|libs|models)/[a-z0-9][a-z0-9-]*|external:[a-z0-9][a-z0-9-]*)`"
)
LINK_RE = re.compile(r"\[[^\]]*\]\(\s*([^\s)]+)")
SCHEME_RE = re.compile(r"^[A-Za-z][A-Za-z0-9+.-]*:")
CODE_SPAN_RE = re.compile(r"(`+)(?:(?!\1).)*?\1")
SELF_GRADE_RE = re.compile(
    r"^\s*(?:[-*]\s+)?(?:Status|Maturity|Production|Feature Class|"
    r"Required Verification)\s*:",
    re.MULTILINE,
)
ISSUE_RE = re.compile(r"#\d+")
CARGO_VALUE_FLAGS = frozenset(
    {
        "-p",
        "--package",
        "--exclude",
        "--features",
        "-F",
        "--test",
        "--bin",
        "--example",
        "--bench",
        "--manifest-path",
        "--target",
        "--profile",
        "--target-dir",
        "-j",
        "--jobs",
    }
)


@dataclass(frozen=True)
class Finding:
    rule: str
    line: int
    message: str

    def as_dict(self) -> dict[str, object]:
        return {"rule": self.rule, "line": self.line, "message": self.message}


@dataclass
class Capability:
    name: str
    capability_id: str
    promise: str
    sources: list[str]
    gates: list[str] = field(default_factory=list)

    def as_dict(self) -> dict[str, object]:
        return {
            "name": self.name,
            "id": self.capability_id,
            "promise": self.promise,
            "sources": self.sources,
            "gates": self.gates,
        }


@dataclass
class Report:
    path: str
    sha256: str
    product_sections: list[str]
    capabilities: list[Capability]
    relative_links: int
    gate_count: int
    warnings: list[str]
    findings: list[Finding]

    @property
    def ok(self) -> bool:
        return not self.findings

    def as_dict(self) -> dict[str, object]:
        return {
            "path": self.path,
            "sha256": self.sha256,
            "ok": self.ok,
            "product_sections": self.product_sections,
            "capabilities": [capability.as_dict() for capability in self.capabilities],
            "relative_links": self.relative_links,
            "gate_count": self.gate_count,
            "warnings": self.warnings,
            "findings": [finding.as_dict() for finding in self.findings],
        }


def usage_error(message: str) -> None:
    print(f"error: {message}", file=sys.stderr)
    raise SystemExit(2)


def repo_root() -> Path:
    proc = subprocess.run(
        [*GIT, "rev-parse", "--show-toplevel"],
        capture_output=True,
        text=True,
        check=False,
    )
    if proc.returncode != 0 or not proc.stdout.strip():
        usage_error(proc.stderr.strip() or "not inside a git checkout")
    return Path(proc.stdout.strip()).resolve()


def resolve_target(repo: Path, raw: str) -> Path:
    path = Path(raw)
    if not path.is_absolute():
        path = repo / path
    path = path.resolve()
    if path.is_dir():
        path = path / "README.md"
    try:
        path.relative_to(repo)
    except ValueError:
        usage_error(f"target is outside the checkout: {raw}")
    if not path.is_file():
        usage_error(f"README does not exist: {path}")
    if path.name != "README.md":
        usage_error(f"target must be a README.md: {path}")
    return path


def line_number(lines: list[str], index: int) -> int:
    return index + 1


def without_fenced_content(lines: list[str]) -> list[str]:
    """Blank fenced examples while preserving line numbers."""

    visible: list[str] = []
    fence = ""
    for line in lines:
        marker = line.lstrip()[:3]
        if marker in {"```", "~~~"}:
            if not fence:
                fence = marker
            elif marker == fence:
                fence = ""
            visible.append("")
            continue
        visible.append("" if fence else line)
    return visible


def section_end(lines: list[str], start: int, level: int) -> int:
    prefix = "#" * level + " "
    for index in range(start + 1, len(lines)):
        if lines[index].startswith(prefix):
            return index
    return len(lines)


def parse_table_row(line: str) -> list[str]:
    return [cell.strip() for cell in line.strip().strip("|").split("|")]


def is_separator_row(cells: list[str]) -> bool:
    return bool(cells) and all(re.fullmatch(r":?-{3,}:?", cell) for cell in cells)


def source_tokens(text: str) -> list[str]:
    return SOURCE_RE.findall(text)


def source_blocks(lines: list[str], start: int, end: int) -> list[tuple[int, str]]:
    blocks: list[tuple[int, list[str]]] = []
    current: tuple[int, list[str]] | None = None
    for index in range(start, end):
        line = lines[index]
        if re.match(r"^\s{2,}-\s+", line):
            if current:
                blocks.append(current)
            current = (index, [line.strip()])
            continue
        if current and (not line.strip() or re.match(r"^\s{4,}\S", line)):
            if line.strip():
                current[1].append(line.strip())
            continue
        if current:
            blocks.append(current)
            current = None
    if current:
        blocks.append(current)
    return [(index, " ".join(parts)) for index, parts in blocks]


def tracked_manifests(repo: Path) -> list[Path]:
    proc = subprocess.run(
        [*GIT, "ls-files", "-z", "--", "*Cargo.toml"],
        cwd=repo,
        capture_output=True,
        text=True,
        check=False,
    )
    if proc.returncode != 0:
        usage_error(f"git ls-files failed: {proc.stderr.strip()}")
    return [repo / rel for rel in proc.stdout.split("\0") if rel]


def cargo_index(repo: Path) -> dict[str, set[str]]:
    index: dict[str, set[str]] = {}
    for manifest_path in tracked_manifests(repo):
        try:
            manifest = tomllib.loads(manifest_path.read_text(encoding="utf-8"))
        except (OSError, tomllib.TOMLDecodeError):
            continue
        package = manifest.get("package")
        if not isinstance(package, dict) or not isinstance(package.get("name"), str):
            continue
        targets = {
            target["name"]
            for target in manifest.get("test", [])
            if isinstance(target, dict) and isinstance(target.get("name"), str)
        }
        if package.get("autotests") is not False:
            targets.update(path.stem for path in manifest_path.parent.glob("tests/*.rs"))
        index[package["name"]] = targets
    return index


def local_package(readme: Path) -> str | None:
    manifest_path = readme.parent / "Cargo.toml"
    if not manifest_path.is_file():
        return None
    try:
        manifest = tomllib.loads(manifest_path.read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError):
        return None
    package = manifest.get("package")
    if isinstance(package, dict) and isinstance(package.get("name"), str):
        return package["name"]
    return None


def validate_cargo_gate(
    command: str,
    line: int,
    package_index: dict[str, set[str]],
    fallback_package: str | None,
) -> list[Finding]:
    try:
        tokens = shlex.split(command)
    except ValueError as error:
        return [Finding("R8", line, f"gate command cannot be parsed: {error}")]
    if len(tokens) < 2 or tokens[0] != "cargo" or tokens[1] != "test":
        return []

    packages: list[str] = []
    targets: list[str] = []
    filters: list[str] = []
    pending = ""
    for token in tokens[2:]:
        if token == "--":
            break
        if pending:
            if pending in {"-p", "--package"}:
                packages.append(token)
            elif pending == "--test":
                targets.append(token)
            pending = ""
            continue
        if token in CARGO_VALUE_FLAGS:
            pending = token
            continue
        if token.startswith("-"):
            continue
        filters.append(token)

    findings: list[Finding] = []
    if pending:
        findings.append(Finding("R8", line, f"gate flag {pending!r} has no value"))
    for name in filters:
        findings.append(
            Finding(
                "R8",
                line,
                f"{name!r} is a bare test-name filter; use --test <target> or --lib",
            )
        )

    effective_packages = packages or ([fallback_package] if fallback_package else [])
    for package in effective_packages:
        if package not in package_index:
            findings.append(
                Finding("R8", line, f"cargo gate names no package {package!r}")
            )
    known = [package for package in effective_packages if package in package_index]
    for target in targets:
        if known and not any(target in package_index[package] for package in known):
            findings.append(
                Finding(
                    "R8",
                    line,
                    f"cargo gate names no --test target {target!r} in "
                    + ", ".join(known),
                )
            )
    return findings


def validate_gate(
    repo: Path,
    readme: Path,
    command: str,
    line: int,
    package_index: dict[str, set[str]],
    warnings: list[str],
) -> list[Finding]:
    findings = validate_cargo_gate(command, line, package_index, local_package(readme))
    try:
        tokens = shlex.split(command)
    except ValueError:
        return findings
    if not tokens:
        return [*findings, Finding("R8", line, "gate command is empty")]
    if tokens[:2] == ["cargo", "test"]:
        return findings

    candidate = ""
    if tokens[0] in {"bash", "sh"} and len(tokens) > 1:
        candidate = tokens[1]
    elif "/" in tokens[0] or tokens[0].endswith(".sh"):
        candidate = tokens[0]
    if candidate:
        path = Path(candidate)
        if not path.is_absolute():
            path = repo / path
        if not path.exists():
            findings.append(
                Finding("R8", line, f"gate script or executable does not exist: {candidate}")
            )
        return findings

    warnings.append(
        f"line {line}: gate command shape was retained but not mechanically resolved: {command}"
    )
    return findings


def validate_readme(
    repo: Path,
    readme: Path,
    package_index: dict[str, set[str]] | None = None,
) -> Report:
    raw = readme.read_bytes()
    text = raw.decode("utf-8")
    lines = text.splitlines()
    prose = without_fenced_content(lines)
    findings: list[Finding] = []
    warnings: list[str] = []
    capabilities: list[Capability] = []
    product_sections: list[str] = []

    h1 = [(index, line[2:].strip()) for index, line in enumerate(prose) if line.startswith("# ")]
    if len(h1) != 1:
        findings.append(Finding("R1", 1, f"README must contain exactly one H1; found {len(h1)}"))

    h2 = [(index, line[3:].strip()) for index, line in enumerate(prose) if line.startswith("## ")]
    positions: dict[str, list[int]] = {}
    for index, name in h2:
        positions.setdefault(name, []).append(index)
    for name in REQUIRED_H2:
        count = len(positions.get(name, []))
        if count != 1:
            findings.append(
                Finding("R1", 1, f"required heading '## {name}' appears {count} times")
            )

    if all(len(positions.get(name, [])) == 1 for name in REQUIRED_H2):
        ordered = [positions[name][0] for name in REQUIRED_H2]
        if ordered != sorted(ordered):
            findings.append(
                Finding("R2", 1, "required H2 sections are not in the contract order")
            )
        workflow_at = positions["Primary workflow"][0]
        discovery_at = positions["Contract discovery"][0]
        product_sections = [
            name for index, name in h2 if workflow_at < index < discovery_at
        ]
        if not product_sections:
            findings.append(
                Finding(
                    "R4",
                    line_number(lines, workflow_at),
                    "add at least one app- or library-specific functional H2 between "
                    "Primary workflow and Contract discovery",
                )
            )

        brief_at = positions["Brief"][0]
        brief_body = [line for line in prose[brief_at + 1 : section_end(prose, brief_at, 2)] if line.strip()]
        if not brief_body:
            findings.append(Finding("R3", line_number(lines, brief_at), "Brief is empty"))

        workflow_body = prose[
            workflow_at + 1 : section_end(prose, workflow_at, 2)
        ]
        steps = [line for line in workflow_body if re.match(r"^\d+\.\s+\S", line)]
        if len(steps) < 2:
            findings.append(
                Finding(
                    "R3",
                    line_number(lines, workflow_at),
                    "Primary workflow must contain at least two numbered steps",
                )
            )

        supporting_at = positions["Supporting documents"][0]
        supporting_text = "\n".join(
            prose[supporting_at + 1 : section_end(prose, supporting_at, 2)]
        )
        if not LINK_RE.search(supporting_text):
            findings.append(
                Finding(
                    "R3",
                    line_number(lines, supporting_at),
                    "Supporting documents must link to at least one maintained document",
                )
            )

    if len(positions.get("Capabilities", [])) == 1:
        cap_at = positions["Capabilities"][0]
        cap_end = section_end(prose, cap_at, 2)
        cap_lines = prose[cap_at + 1 : cap_end]
        cap_text = "\n".join(cap_lines)

        if SELF_GRADE_RE.search(cap_text):
            findings.append(
                Finding("R10", line_number(lines, cap_at), "Capabilities contains a self-graded field")
            )
        if ISSUE_RE.search(cap_text):
            findings.append(
                Finding("R10", line_number(lines, cap_at), "Capabilities must not cite work-item numbers")
            )
        if re.search(r"^\s*[-*]\s+Root WI\s*:", cap_text, re.MULTILINE):
            findings.append(
                Finding("R10", line_number(lines, cap_at), "Capabilities must not contain Root WI fields")
            )
        for offset, line in enumerate(cap_lines):
            if line.startswith("#### "):
                findings.append(
                    Finding(
                        "R12",
                        cap_at + offset + 2,
                        "capabilities are flat H3 entries; H4 capability hierarchy is not allowed",
                    )
                )

        h3 = [
            (index, prose[index][4:].strip())
            for index in range(cap_at + 1, cap_end)
            if prose[index].startswith("### ")
        ]
        if not h3 or h3[0][1] != "Capability index":
            findings.append(
                Finding("R5", line_number(lines, cap_at), "Capabilities must start with '### Capability index'")
            )
        else:
            index_at = h3[0][0]
            table_lines: list[tuple[int, str]] = []
            started = False
            for index in range(index_at + 1, h3[1][0] if len(h3) > 1 else cap_end):
                if prose[index].lstrip().startswith("|"):
                    table_lines.append((index, prose[index]))
                    started = True
                elif started and prose[index].strip():
                    break
            rows: list[tuple[int, list[str]]] = [
                (index, parse_table_row(line)) for index, line in table_lines
            ]
            if len(rows) < 3 or tuple(rows[0][1]) != CAPABILITY_HEADER or not is_separator_row(rows[1][1]):
                findings.append(
                    Finding(
                        "R5",
                        line_number(lines, index_at),
                        "Capability index must use columns: Capability | ID | User promise | Sources",
                    )
                )
                rows = []

            seen_ids: set[str] = set()
            seen_names: set[str] = set()
            indexed: list[tuple[int, Capability]] = []
            for row_line, cells in rows[2:]:
                if len(cells) != 4:
                    findings.append(
                        Finding("R5", line_number(lines, row_line), "Capability index row must have four cells")
                    )
                    continue
                name, raw_id, promise, raw_sources = cells
                match = re.fullmatch(r"`([^`]+)`", raw_id)
                capability_id = match.group(1) if match else ""
                sources = source_tokens(raw_sources)
                if not name or not promise:
                    findings.append(
                        Finding("R5", line_number(lines, row_line), "Capability name and promise cannot be empty")
                    )
                if not match or not ID_RE.fullmatch(capability_id):
                    findings.append(
                        Finding("R5", line_number(lines, row_line), f"invalid capability ID: {raw_id}")
                    )
                if name in seen_names or capability_id in seen_ids:
                    findings.append(
                        Finding("R5", line_number(lines, row_line), "Capability names and IDs must be unique")
                    )
                if not sources:
                    findings.append(
                        Finding("R7", line_number(lines, row_line), "Capability index row has no source")
                    )
                if len(sources) != len(set(sources)):
                    findings.append(
                        Finding("R7", line_number(lines, row_line), "Capability index row repeats a source")
                    )
                seen_names.add(name)
                seen_ids.add(capability_id)
                indexed.append((row_line, Capability(name, capability_id, promise, sources)))

            detail_headings = h3[1:]
            detail_names = [name for _, name in detail_headings]
            index_names = [capability.name for _, capability in indexed]
            if detail_names != index_names:
                findings.append(
                    Finding(
                        "R6",
                        line_number(lines, index_at),
                        "Capability detail headings must exactly match the index order",
                    )
                )

            package_index = package_index if package_index is not None else cargo_index(repo)
            detail_by_name = {name: position for position, name in detail_headings}
            for _row_line, capability in indexed:
                start = detail_by_name.get(capability.name)
                if start is None:
                    continue
                next_positions = [position for position, _name in detail_headings if position > start]
                end = min(next_positions) if next_positions else cap_end
                detail = "\n".join(prose[start + 1 : end])

                id_match = re.search(r"^- ID:\s*`([^`]+)`\s*$", detail, re.MULTILINE)
                if not id_match or id_match.group(1) != capability.capability_id:
                    findings.append(
                        Finding(
                            "R6",
                            line_number(lines, start),
                            f"detail ID must be `{capability.capability_id}`",
                        )
                    )
                if not re.search(r"^- Promise:\s*\S", detail, re.MULTILINE):
                    findings.append(
                        Finding("R6", line_number(lines, start), "capability detail has no Promise")
                    )

                source_marker = next(
                    (index for index in range(start + 1, end) if prose[index].strip() == "- Sources:"),
                    None,
                )
                detail_sources: list[str] = []
                if source_marker is None:
                    findings.append(
                        Finding("R7", line_number(lines, start), "capability detail has no Sources list")
                    )
                else:
                    source_end = next(
                        (
                            index
                            for index in range(source_marker + 1, end)
                            if re.match(r"^- [A-Z][A-Za-z ]*:", prose[index])
                        ),
                        end,
                    )
                    for block_at, block in source_blocks(prose, source_marker + 1, source_end):
                        tokens = source_tokens(block)
                        if len(tokens) != 1:
                            findings.append(
                                Finding(
                                    "R7",
                                    line_number(lines, block_at),
                                    "each source bullet must name exactly one apps/, libs/, or external: source",
                                )
                            )
                            continue
                        source = tokens[0]
                        detail_sources.append(source)
                        contribution = re.sub(r"\[[^\]]+\]\([^)]+\)", "", block)
                        contribution = contribution.replace(f"`{source}`", "")
                        contribution = re.sub(r"^\s*-\s*", "", contribution).strip(" .")
                        if len(contribution.split()) < 3:
                            findings.append(
                                Finding(
                                    "R7",
                                    line_number(lines, block_at),
                                    f"source {source!r} must state its direct contribution",
                                )
                            )
                        if source.startswith(("apps/", "libs/", "models/")) and not (repo / source).exists():
                            findings.append(
                                Finding(
                                    "R7",
                                    line_number(lines, block_at),
                                    f"source path does not exist: {source}",
                                )
                            )
                if detail_sources != capability.sources:
                    findings.append(
                        Finding(
                            "R7",
                            line_number(lines, start),
                            "detail Sources must exactly match the Capability index sources",
                        )
                    )

                gates: list[str] = []
                for index in range(start + 1, end):
                    if not prose[index].startswith("- Gate:"):
                        continue
                    match = re.fullmatch(r"- Gate:\s*`([^`]+)`\s*", prose[index])
                    if not match:
                        findings.append(
                            Finding("R8", line_number(lines, index), "Gate must contain one backticked command")
                        )
                        continue
                    command = match.group(1)
                    gates.append(command)
                    findings.extend(
                        validate_gate(
                            repo,
                            readme,
                            command,
                            line_number(lines, index),
                            package_index,
                            warnings,
                        )
                    )
                if not gates:
                    findings.append(
                        Finding("R8", line_number(lines, start), "capability detail has no executable Gate")
                    )
                capability.gates = gates
                capabilities.append(capability)

    relative_links = 0
    in_fence = False
    fence = ""
    for index, line in enumerate(lines):
        marker = line.lstrip()[:3]
        if marker in {"```", "~~~"}:
            if not in_fence:
                in_fence = True
                fence = marker
            elif marker == fence:
                in_fence = False
                fence = ""
            continue
        if in_fence:
            continue
        prose = CODE_SPAN_RE.sub(lambda match: " " * len(match.group(0)), line)
        for raw_target in LINK_RE.findall(prose):
            if not raw_target or raw_target.startswith(("#", "/")) or SCHEME_RE.match(raw_target):
                continue
            if any(char in raw_target for char in "<>{}"):
                continue
            relative_links += 1
            target = raw_target.split("#", 1)[0].split("?", 1)[0]
            if target and not (readme.parent / target).exists():
                findings.append(
                    Finding("R9", line_number(lines, index), f"relative link target does not exist: {raw_target}")
                )
    if in_fence:
        findings.append(Finding("R11", len(lines), "fenced code block is not closed"))

    findings.sort(key=lambda finding: (finding.line, finding.rule, finding.message))
    return Report(
        path=str(readme.relative_to(repo)),
        sha256=hashlib.sha256(raw).hexdigest(),
        product_sections=product_sections,
        capabilities=capabilities,
        relative_links=relative_links,
        gate_count=sum(len(capability.gates) for capability in capabilities),
        warnings=warnings,
        findings=findings,
    )


def print_text(reports: list[Report]) -> None:
    for report in reports:
        state = "PASS" if report.ok else "FAIL"
        print(f"{state} {report.path} sha256={report.sha256}")
        print(
            "  "
            f"product_sections={len(report.product_sections)} "
            f"capabilities={len(report.capabilities)} "
            f"relative_links={report.relative_links} "
            f"gates={report.gate_count}"
        )
        for warning in report.warnings:
            print(f"  WARN {warning}")
        for finding in report.findings:
            print(f"  {finding.rule} line {finding.line}: {finding.message}")


def clean_reader_prompt(report: Report, absolute_path: Path) -> str:
    return f"""Read only this file: {absolute_path}

Do not inspect any other file, repository state, prior draft, conversation, or
validator output. Do not edit anything. Compute the file SHA-256 yourself. The
expected SHA-256 is {report.sha256}. Stop and report stale_input if it differs.

Act as a first-time developer who must decide whether this README is enough to
understand the product and find the detailed contracts. Return JSON only:

{{
  "sha256": "<computed sha256>",
  "status": "reviewed | stale_input",
  "stated": {{
    "purpose": "<what the product is>",
    "not_for": ["<explicit product boundary>"],
    "primary_workflow": ["<ordered steps>"],
    "functional_surfaces": ["<main user-facing functions>"],
    "capabilities": [
      {{"name": "<heading>", "id": "<id>", "sources": ["<source>"]}}
    ],
    "source_model": {{
      "apps": "<meaning>",
      "libs": "<meaning>",
      "external": "<meaning>"
    }}
  }},
  "inferences": ["<clearly labelled inference>"],
  "blocking_contradictions": ["<contradiction that blocks comprehension>"],
  "missing_details": ["<detail intentionally delegated or still hard to find>"],
  "comprehension_score": 0
}}

List every capability. Keep facts from the README separate from inferences.
The score is diagnostic. Do not turn it into the verdict.
"""


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)

    check_parser = subparsers.add_parser("check", help="run deterministic checks")
    check_parser.add_argument("paths", nargs="+", help="README path or project directory")
    check_parser.add_argument("--format", choices=("text", "json"), default="text")

    prompt_parser = subparsers.add_parser("prompt", help="emit the clean-reader task")
    prompt_parser.add_argument("path", help="README path or project directory")

    args = parser.parse_args(argv)
    repo = repo_root()
    package_index = cargo_index(repo)

    if args.command == "check":
        reports = [
            validate_readme(repo, resolve_target(repo, raw), package_index)
            for raw in args.paths
        ]
        if args.format == "json":
            print(json.dumps({"reports": [report.as_dict() for report in reports]}, indent=2))
        else:
            print_text(reports)
        return 0 if all(report.ok for report in reports) else 1

    target = resolve_target(repo, args.path)
    report = validate_readme(repo, target, package_index)
    if not report.ok:
        print_text([report])
        print("error: clean-reader review is skipped until deterministic findings are fixed", file=sys.stderr)
        return 1
    print(clean_reader_prompt(report, target))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
