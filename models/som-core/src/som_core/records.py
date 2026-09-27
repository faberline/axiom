"""Layer records: the L1 plan, L2 topology, and L3 operation list.

``docs/reference/layer-records.md`` is the specification; this module is its
consumer and refuses what the document does not allow. Records are plain JSON
values (dicts and lists), validated where they enter: the loader, the
assembler, and the corpus project's curation check.

Rules this module owns:

- A plan is ``{intent, target, constraints}``, each non-empty.
- A topology is at most :data:`MAX_FILES` files and :data:`MAX_BLOCKS`
  blocks in total; a larger one is the planner's ``SCOPE_TOO_LARGE``
  refusal (:class:`ScopeTooLarge`), checked before anything else so an
  oversized plan never reaches a lower layer.
- Every ``depends_on`` entry names a declared block: a bare name for the
  same file, ``<path>:<name>`` for another.
- An operation list starts each file with ``CREATE_FILE``, keeps each
  file's imports in non-decreasing ``group`` order, and inserts every
  declared block exactly once, in topology order. An operation carries
  exactly its fields, so a misspelt key is refused rather than ignored.
"""

from __future__ import annotations

from typing import Any

MAX_FILES = 5
MAX_BLOCKS = 20
MAX_BLANK_BEFORE = 2

KINDS = frozenset({"class", "function", "route", "command", "constant", "statement"})

OP_FIELDS: dict[str, frozenset[str]] = {
    "CREATE_FILE": frozenset({"op", "path", "docstring"}),
    "ADD_IMPORT": frozenset({"op", "path", "stmt", "group"}),
    "INSERT_SNIPPET": frozenset({"op", "path", "block", "id", "params", "blank_before"}),
    "INSERT_BLOCK": frozenset({"op", "path", "block", "source", "blank_before"}),
}
INSERT_OPS = frozenset({"INSERT_SNIPPET", "INSERT_BLOCK"})


class RecordError(ValueError):
    """A layer record that the specification does not allow."""


class ScopeTooLarge(RecordError):
    """A topology over the planner's scope limit."""

    code = "SCOPE_TOO_LARGE"


def _text(value: Any, where: str) -> str:
    if not isinstance(value, str) or not value.strip():
        raise RecordError(f"{where}: must be a non-empty string")
    return value


def _is_section(value: Any) -> bool:
    return isinstance(value, list) and all(
        isinstance(item, dict)
        and all(isinstance(k, str) and isinstance(v, str) for k, v in item.items())
        for item in value
    )


def _int(value: Any, where: str, low: int, high: int | None = None) -> int:
    if not isinstance(value, int) or isinstance(value, bool) or value < low or (
        high is not None and value > high
    ):
        span = f"{low}..{high}" if high is not None else f">= {low}"
        raise RecordError(f"{where}: must be an integer {span}")
    return value


def validate_plan(plan: Any) -> None:
    """Refuse an L1 plan that is not ``{intent, target, constraints}``."""
    if not isinstance(plan, dict):
        raise RecordError("plan: must be an object")
    if set(plan) != {"intent", "target", "constraints"}:
        raise RecordError(f"plan: fields must be intent, target, constraints; got {sorted(plan)}")
    _text(plan["intent"], "plan.intent")
    _text(plan["target"], "plan.target")
    constraints = plan["constraints"]
    if not isinstance(constraints, list) or not constraints:
        raise RecordError("plan.constraints: must be a non-empty list")
    for i, item in enumerate(constraints):
        _text(item, f"plan.constraints[{i}]")


def check_scope(topology: dict[str, Any]) -> None:
    """Refuse a topology over :data:`MAX_FILES` files or :data:`MAX_BLOCKS` blocks."""
    files = topology.get("files") or []
    blocks = sum(len(f.get("blocks") or []) for f in files if isinstance(f, dict))
    if len(files) > MAX_FILES or blocks > MAX_BLOCKS:
        raise ScopeTooLarge(
            f"SCOPE_TOO_LARGE: {len(files)} files and {blocks} blocks "
            f"(limit {MAX_FILES} files, {MAX_BLOCKS} blocks)"
        )


def validate_topology(topology: Any) -> None:
    """Refuse an L2 topology the specification does not allow."""
    if not isinstance(topology, dict) or set(topology) != {"files"}:
        raise RecordError("topology: must be an object with exactly 'files'")
    files = topology["files"]
    if not isinstance(files, list) or not files:
        raise RecordError("topology.files: must be a non-empty list")
    check_scope(topology)

    declared: dict[str, list[str]] = {}
    for fi, entry in enumerate(files):
        where = f"topology.files[{fi}]"
        if not isinstance(entry, dict) or set(entry) != {"path", "blocks"}:
            raise RecordError(f"{where}: must be an object with exactly 'path' and 'blocks'")
        path = _text(entry["path"], f"{where}.path")
        if not path.endswith(".py") or path.startswith("/") or ".." in path.split("/"):
            raise RecordError(f"{where}.path: {path!r} must be a relative .py path")
        if path in declared:
            raise RecordError(f"{where}.path: {path!r} is declared twice")
        blocks = entry["blocks"]
        if not isinstance(blocks, list):
            raise RecordError(f"{where}.blocks: must be a list")
        names: list[str] = []
        for bi, block in enumerate(blocks):
            bwhere = f"{where}.blocks[{bi}]"
            if not isinstance(block, dict) or set(block) != {"name", "kind", "depends_on"}:
                raise RecordError(f"{bwhere}: fields must be name, kind, depends_on")
            name = _text(block["name"], f"{bwhere}.name")
            if name in names:
                raise RecordError(f"{bwhere}.name: {name!r} is declared twice in {path}")
            if block["kind"] not in KINDS:
                raise RecordError(f"{bwhere}.kind: {block['kind']!r} is not one of {sorted(KINDS)}")
            if not isinstance(block["depends_on"], list):
                raise RecordError(f"{bwhere}.depends_on: must be a list")
            names.append(name)
        declared[path] = names

    for entry in files:
        path = entry["path"]
        for block in entry["blocks"]:
            for dep in block["depends_on"]:
                dep_path, _, dep_name = dep.rpartition(":") if ":" in dep else (path, "", dep)
                if dep_name not in declared.get(dep_path, []):
                    raise RecordError(
                        f"topology: block {block['name']!r} in {path} depends on "
                        f"undeclared block {dep!r}"
                    )


def validate_ops(ops: Any, topology: dict[str, Any]) -> None:
    """Refuse an L3 operation list that does not realise ``topology``."""
    validate_topology(topology)
    if not isinstance(ops, list) or not ops:
        raise RecordError("ops: must be a non-empty list")
    order = {f["path"]: [b["name"] for b in f["blocks"]] for f in topology["files"]}
    created: set[str] = set()
    last_group: dict[str, int] = {}
    inserted: dict[str, list[str]] = {path: [] for path in order}

    for i, op in enumerate(ops):
        where = f"ops[{i}]"
        if not isinstance(op, dict) or op.get("op") not in OP_FIELDS:
            raise RecordError(f"{where}: unknown operation {op.get('op') if isinstance(op, dict) else op!r}")
        kind = op["op"]
        if set(op) != OP_FIELDS[kind]:
            raise RecordError(f"{where}: {kind} fields must be {sorted(OP_FIELDS[kind])}; got {sorted(op)}")
        path = op["path"]
        if path not in order:
            raise RecordError(f"{where}: {kind} names undeclared file {path!r}")

        if kind == "CREATE_FILE":
            if path in created:
                raise RecordError(f"{where}: {path!r} is created twice")
            if op["docstring"] is not None and not isinstance(op["docstring"], str):
                raise RecordError(f"{where}.docstring: must be a string or null")
            created.add(path)
            continue
        if path not in created:
            raise RecordError(f"{where}: {kind} on {path!r} before its CREATE_FILE")

        if kind == "ADD_IMPORT":
            stmt = _text(op["stmt"], f"{where}.stmt")
            if not stmt.startswith(("import ", "from ")):
                raise RecordError(f"{where}.stmt: {stmt!r} is not an import statement")
            group = _int(op["group"], f"{where}.group", 0)
            if group < last_group.get(path, 0):
                raise RecordError(f"{where}.group: {group} follows group {last_group[path]} in {path}")
            last_group[path] = group
            continue

        block = op["block"]
        if block not in order[path]:
            raise RecordError(f"{where}: {kind} names undeclared block {block!r} in {path}")
        if block in inserted[path]:
            raise RecordError(f"{where}: block {block!r} in {path} is inserted twice")
        expected = order[path][len(inserted[path])]
        if block != expected:
            raise RecordError(f"{where}: block {block!r} in {path} is out of topology order; expected {expected!r}")
        inserted[path].append(block)
        _int(op["blank_before"], f"{where}.blank_before", 0, MAX_BLANK_BEFORE)
        if kind == "INSERT_SNIPPET":
            _text(op["id"], f"{where}.id")
            params = op["params"]
            if not isinstance(params, dict) or not all(
                isinstance(k, str) and (isinstance(v, str) or _is_section(v))
                for k, v in params.items()
            ):
                raise RecordError(
                    f"{where}.params: must map names to strings or to lists of string maps"
                )
        else:
            _text(op["source"], f"{where}.source")

    missing = sorted(path for path in order if path not in created)
    if missing:
        raise RecordError(f"ops: no CREATE_FILE for {missing}")
    for path, names in order.items():
        if inserted[path] != names:
            left = [n for n in names if n not in inserted[path]]
            raise RecordError(f"ops: blocks never inserted in {path}: {left}")


def coverage(ops: list[dict[str, Any]]) -> dict[str, int]:
    """Count snippet and literal block insertions."""
    return {
        "snippet_blocks": sum(1 for op in ops if op["op"] == "INSERT_SNIPPET"),
        "literal_blocks": sum(1 for op in ops if op["op"] == "INSERT_BLOCK"),
    }
