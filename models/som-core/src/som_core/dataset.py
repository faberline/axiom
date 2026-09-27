"""Corpus loader for the Snippet-Oriented Model (SOM).

Parses oracle-family corpora (``families/<family>/family.json`` plus one
candidate file per entry) into rows the layer models train on. A row is not
a multiple-choice question: the gold candidate is what the layers learn to
rebuild, and the near misses ride along as negative examples.

Rules this module owns:

- A corpus is one directory whose ``families/`` holds the families; several
  corpora (``som-code``, ``som-code-python``, ``som-code-rust``) are loaded
  together by :func:`load_corpora`, which refuses an empty corpus and a
  family id that two corpora both claim, so a union is never silently a
  subset and a row id is never ambiguous.
- Each corpus project ships two layers: ``data/curated`` (the project's own,
  checked by its harness and curation gates) and ``data/user`` (families a
  user adds locally, same schema, never committed). With no ``--data-dir``,
  :func:`find_default_corpora` loads curated first and user only when it
  holds a family; the duplicate-id refusal keeps a user family from
  shadowing a curated one.
- Every dict row carries ``layer``: ``"user"`` when its corpus directory is
  named ``user``, ``"curated"`` otherwise. A user family replaces a curated
  one only by declaring ``"overrides": "<row id>"`` in its ``family.json``;
  the target must already be loaded, only the user layer may override, and
  one target takes one override, so a team convention wins on purpose and a
  typo never silently leaves the curated family in place.
- User rows are team conventions and train with more weight:
  :data:`DEFAULT_LAYER_WEIGHTS` gives each layer's expected repeats per
  epoch, applied by :func:`weighted_epoch` to the training split only.
  :func:`split_dataset` stratifies by layer, so each layer has its own
  unweighted validation rows and its accuracy is reported on its own.
- The curation fields a corpus project measures for each near miss,
  ``why_wrong`` and ``caught_by``, ride on :class:`CandidateRecord` unchanged
  and reach the training row through :meth:`CandidateRecord.to_dict`; a gold
  candidate carries neither. The engine never rewrites them.
- Family-level curation (``rationale``, ``oracle``, ``caption``, ``plan``,
  ``decompiled``) reaches the row through ``metadata`` with every other
  ``family.json`` key that is not a candidate, the requirement, or the
  skeleton. ``caption`` is the planner's prompt, ``plan`` the L1 record, and
  ``decompiled.topology`` / ``decompiled.ops`` the L2 and L3 records (see
  ``docs/reference/layer-records.md``); the corpus project authors or
  measures and checks them, and the engine passes them through untouched.
"""

from __future__ import annotations

from dataclasses import dataclass, field
import json
import math
import os
from pathlib import Path
import random
from typing import Any, Iterable, Optional

CURATED = "curated"
USER = "user"
FAMILIES = "families"
LAYERS = (CURATED, USER)
DEFAULT_LAYER_WEIGHTS: dict[str, float] = {CURATED: 1.0, USER: 2.0}


@dataclass(frozen=True)
class CandidateRecord:
    """A single candidate implementation within a scenario family.

    ``why_wrong`` is the corpus author's one-sentence observable consequence
    of a near miss's defect; ``caught_by`` is the sorted tuple of fixture
    tests the corpus measured failing on it, or ``None`` when the family has
    no oracle. Both are ``None`` on a gold candidate.
    """
    id: str
    text: str
    kind: str  # "gold" or "near_miss"
    failure_mode: Optional[str] = None
    oracle_pass: bool = False
    module: Optional[str] = None
    why_wrong: Optional[str] = None
    caught_by: Optional[tuple[str, ...]] = None

    def to_dict(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "id": self.id,
            "text": self.text,
            "kind": self.kind,
            "oracle_pass": self.oracle_pass,
        }
        if self.failure_mode:
            result["failure_mode"] = self.failure_mode
        if self.module:
            result["module"] = self.module
        if self.why_wrong is not None:
            result["why_wrong"] = self.why_wrong
        if self.caught_by is not None:
            result["caught_by"] = list(self.caught_by)
        return result


@dataclass
class ScenarioFamily:
    """A scenario family containing requirements, skeleton, and candidate pool."""
    family_id: str
    domain: str
    area: str
    capability: str
    requirement: str
    skeleton: str
    candidates: list[CandidateRecord]
    gold_candidate_id: str = "gold"
    metadata: dict[str, Any] = field(default_factory=dict)

    def to_row(self) -> dict[str, Any]:
        """The training row: family identity, the requirement, candidates, and curation."""
        return {
            "id": f"som:{self.domain}:{self.family_id}",
            "family": self.family_id,
            "domain": self.domain,
            "area": self.area,
            "capability": self.capability,
            "requirement": self.requirement,
            "skeleton": self.skeleton,
            "candidates": [c.to_dict() for c in self.candidates],
            "gold_candidate_id": self.gold_candidate_id,
            "metadata": {
                **self.metadata,
                "family_id": self.family_id,
                "domain": self.domain,
                "area": self.area,
                "capability": self.capability,
            },
        }


def load_family_from_metadata(family_dir: Path | str) -> ScenarioFamily:
    """Parse a single family directory containing family.json and candidate files.

    Parameters
    ----------
    family_dir : Path | str
        Path to the family directory or family.json file.

    Returns
    -------
    ScenarioFamily
        The parsed scenario family with loaded candidate source code.
    """
    path = Path(family_dir).resolve()
    if path.is_file() and path.name == "family.json":
        family_dir_path = path.parent
        meta_file = path
    elif path.is_dir():
        family_dir_path = path
        meta_file = path / "family.json"
        if not meta_file.exists():
            raise FileNotFoundError(f"Missing family.json in directory: {path}")
    else:
        raise FileNotFoundError(f"Invalid family directory or file: {path}")

    meta = json.loads(meta_file.read_text(encoding="utf-8"))

    family_id = meta.get("family_id", family_dir_path.name)
    domain = meta.get("domain", "python")
    area = meta.get("area", "backend")
    capability = meta.get("capability", "general")
    requirement = meta.get("requirement", "")
    skeleton = meta.get("skeleton", "")

    raw_candidates = meta.get("candidates", [])
    if not isinstance(raw_candidates, list) or len(raw_candidates) < 2:
        raise ValueError(f"Family {family_id} must have at least 2 candidates, got {len(raw_candidates)}")

    parsed_candidates: list[CandidateRecord] = []
    gold_id: Optional[str] = None

    for cand_meta in raw_candidates:
        cid = cand_meta.get("id")
        if not cid or not isinstance(cid, str):
            raise ValueError(f"Candidate missing id in {family_id}: {cand_meta}")

        kind = cand_meta.get("kind", "gold" if cid == "gold" else "near_miss")
        failure_mode = cand_meta.get("failure_mode")
        oracle_pass = (kind == "gold" or cid == "gold")
        why_wrong = cand_meta.get("why_wrong")
        if why_wrong is not None and not isinstance(why_wrong, str):
            raise ValueError(f"Candidate '{cid}' in {family_id}: why_wrong must be a string")
        raw_caught_by = cand_meta.get("caught_by")
        caught_by: Optional[tuple[str, ...]] = None
        if raw_caught_by is not None:
            if not isinstance(raw_caught_by, list) or not all(isinstance(t, str) for t in raw_caught_by):
                raise ValueError(f"Candidate '{cid}' in {family_id}: caught_by must be a list of test names")
            caught_by = tuple(raw_caught_by)

        # Resolve candidate code file
        module_rel = cand_meta.get("module")
        code_file = None
        if module_rel:
            cand_path = family_dir_path / module_rel
            if cand_path.exists():
                code_file = cand_path

        if code_file is None:
            # Fallback search locations
            candidates_to_try = [
                family_dir_path / cid / "candidate.py",
                family_dir_path / f"{cid}.py",
                family_dir_path / "candidate.py",
            ]
            for candidate_try in candidates_to_try:
                if candidate_try.exists():
                    code_file = candidate_try
                    break

        if code_file is None:
            raise FileNotFoundError(
                f"Candidate code file not found for candidate '{cid}' in {family_dir_path}"
            )

        code_text = code_file.read_text(encoding="utf-8").strip()
        if not code_text:
            raise ValueError(f"Candidate code in {code_file} is empty")

        parsed_candidates.append(
            CandidateRecord(
                id=cid,
                text=code_text,
                kind=kind,
                failure_mode=failure_mode,
                oracle_pass=oracle_pass,
                module=module_rel or str(code_file.relative_to(family_dir_path)),
                why_wrong=why_wrong,
                caught_by=caught_by,
            )
        )

        if oracle_pass and gold_id is None:
            gold_id = cid

    if gold_id is None:
        raise ValueError(f"No gold candidate identified in family {family_id}")

    return ScenarioFamily(
        family_id=family_id,
        domain=domain,
        area=area,
        capability=capability,
        requirement=requirement,
        skeleton=skeleton,
        candidates=parsed_candidates,
        gold_candidate_id=gold_id,
        metadata={k: v for k, v in meta.items() if k not in ("candidates", "requirement", "skeleton")},
    )


def load_families(families_dir: Path | str) -> list[ScenarioFamily]:
    """Scan a ``families/`` directory and parse all scenario families.

    Parameters
    ----------
    families_dir : Path | str
        Path to a families directory (e.g. data/curated/families).

    Returns
    -------
    list[ScenarioFamily]
        Sorted list of parsed scenario families.
    """
    path = Path(families_dir).resolve()
    if not path.is_dir():
        raise NotADirectoryError(f"Families directory does not exist: {path}")

    family_dirs = [d for d in path.iterdir() if d.is_dir() and (d / "family.json").is_file()]
    if not family_dirs:
        family_dirs = [fj.parent for fj in path.rglob("family.json")]

    family_dirs = sorted(set(family_dirs), key=lambda d: d.name)
    scenarios: list[ScenarioFamily] = []
    for fdir in family_dirs:
        scenarios.append(load_family_from_metadata(fdir))

    return scenarios


def find_default_corpora(search_root: Path | str | None = None) -> list[Path]:
    """Locate som-code-python's corpora: ``data/curated``, then ``data/user``.

    ``data/curated`` is required; ``data/user`` is included only when it
    holds at least one family, so an untouched checkout trains on the
    curated corpus alone. ``SOM_DATA_DIRS`` (``os.pathsep``-separated)
    overrides the search.
    """
    env = os.environ.get("SOM_DATA_DIRS")
    if env:
        return [Path(p).resolve() for p in env.split(os.pathsep) if p]

    start = Path(search_root).resolve() if search_root else Path.cwd().resolve()
    roots = [
        start / "models" / "som-code-python" / "data",
        start / "som-code-python" / "data",
        start / "data",
        start.parent / "som-code-python" / "data",
        Path(__file__).resolve().parents[3] / "som-code-python" / "data",
    ]
    for root in roots:
        curated = root / CURATED
        if (curated / FAMILIES).is_dir():
            dirs = [curated.resolve()]
            user_families = root / USER / FAMILIES
            if user_families.is_dir() and any(user_families.glob("*/family.json")):
                dirs.append((root / USER).resolve())
            return dirs

    raise FileNotFoundError(
        "Could not locate 'som-code-python/data/curated'. "
        "Pass --data-dir explicitly or set SOM_DATA_DIRS."
    )


def load_corpus(
    data_dir: Path | str,
    as_dicts: bool = True,
) -> list[dict[str, Any]] | list[ScenarioFamily]:
    """Load one corpus: a directory whose ``families/`` holds the families."""
    base_path = Path(data_dir).resolve()

    families_dir = base_path / FAMILIES
    scenarios = load_families(families_dir if families_dir.is_dir() else base_path)
    if as_dicts:
        return [s.to_row() for s in scenarios]
    return scenarios


def corpus_layer(corpus: Path | str) -> str:
    """``"user"`` for a corpus directory named ``user``, else ``"curated"``."""
    return USER if Path(corpus).name == USER else CURATED


def load_corpora(
    data_dirs: Iterable[Path | str] | None = None,
    as_dicts: bool = True,
) -> list[dict[str, Any]] | list[ScenarioFamily]:
    """Load one or more corpora into a single dataset, in the order given.

    ``None`` or an empty iterable falls back to :func:`find_default_corpora`.
    A corpus that yields no family, and a row id that two corpora both
    produce, are refused with ``ValueError`` naming the directories, so a
    training run over three corpora is never silently a run over two. A
    user-layer family that declares ``overrides`` replaces that earlier row;
    an override from the curated layer, of an id not yet loaded, or of a
    target another family already overrides is refused.
    """
    dirs = [Path(d) for d in (data_dirs or [])] or find_default_corpora()

    rows: list[Any] = []
    seen: dict[str, Path] = {}
    overridden: dict[str, tuple[str, Path]] = {}
    for corpus in dirs:
        loaded = load_corpus(corpus, as_dicts=as_dicts)
        if not loaded:
            raise ValueError(f"Corpus yields no family: {corpus}")
        layer = corpus_layer(corpus)
        for item in loaded:
            row = item if as_dicts else item.to_row()
            row_id = row["id"]
            if row_id in seen:
                raise ValueError(
                    f"Family id {row_id!r} is claimed by both {seen[row_id]} and {corpus}"
                )
            target = (row.get("metadata") or {}).get("overrides")
            if target is not None:
                if layer != USER:
                    raise ValueError(
                        f"Family {row_id!r} in {corpus} declares overrides {target!r}; "
                        "only the user layer may override"
                    )
                if target in overridden:
                    prior_id, prior_dir = overridden[target]
                    raise ValueError(
                        f"Family id {target!r} is overridden by both {prior_id!r} in {prior_dir} "
                        f"and {row_id!r} in {corpus}"
                    )
                if target not in seen:
                    raise ValueError(
                        f"Family {row_id!r} in {corpus} overrides {target!r}, "
                        "which no earlier corpus loads"
                    )
                overridden[target] = (row_id, corpus)
            if as_dicts:
                item["layer"] = layer
            seen[row_id] = corpus
        rows.extend(loaded)

    if overridden:
        rows = [
            r for r in rows
            if (r["id"] if as_dicts else r.to_row()["id"]) not in overridden
        ]
    return rows


def parse_layer_weights(specs: Iterable[str] | str | None) -> dict[str, float]:
    """Parse ``layer=weight`` specs over :data:`DEFAULT_LAYER_WEIGHTS`.

    Accepts a list of specs or one comma-separated string (the
    ``SOM_LAYER_WEIGHTS`` form). An unknown layer, a non-number, and a
    negative weight are refused; ``0`` keeps the layer out of training but
    not out of validation.
    """
    weights = dict(DEFAULT_LAYER_WEIGHTS)
    if specs is None:
        return weights
    items = specs.split(",") if isinstance(specs, str) else list(specs)
    for spec in items:
        spec = spec.strip()
        if not spec:
            continue
        name, sep, value = spec.partition("=")
        name = name.strip()
        if not sep or name not in LAYERS:
            raise ValueError(f"Layer weight {spec!r} must be one of {', '.join(LAYERS)}=<weight>")
        try:
            weight = float(value)
        except ValueError:
            raise ValueError(f"Layer weight {spec!r} is not a number") from None
        if not (weight >= 0 and math.isfinite(weight)):
            raise ValueError(f"Layer weight {spec!r} must be zero or more and finite")
        weights[name] = weight
    return weights


def resolve_layer_weights(specs: Iterable[str] | None = None) -> dict[str, float]:
    """CLI specs when given, else ``SOM_LAYER_WEIGHTS``, else the defaults."""
    specs = list(specs or [])
    if specs:
        return parse_layer_weights(specs)
    return parse_layer_weights(os.environ.get("SOM_LAYER_WEIGHTS"))


def weighted_epoch(
    rows: list[dict[str, Any]],
    weights: dict[str, float] | None = None,
    seed: int = 42,
) -> list[dict[str, Any]]:
    """Repeat each row by its layer's weight, then shuffle deterministically.

    A weight's integer part is repeated outright and its fractional part adds
    one more copy with that probability, so the expected count per row equals
    the weight. A row without ``layer`` counts as curated.
    """
    weights = weights or DEFAULT_LAYER_WEIGHTS
    rng = random.Random(seed)
    out: list[dict[str, Any]] = []
    for row in rows:
        weight = weights.get(row.get("layer", CURATED), 1.0)
        whole = int(weight)
        copies = whole + (1 if rng.random() < weight - whole else 0)
        out.extend([row] * copies)
    rng.shuffle(out)
    return out


def split_dataset(
    rows: list[Any],
    val_ratio: float = 0.25,
    seed: int = 42,
    stratify_by: str | None = "layer",
) -> tuple[list[Any], list[Any]]:
    """Deterministically split dataset into (train_rows, val_rows).

    With ``stratify_by`` and more than one value of it among the rows, each
    group is split on its own and the halves are concatenated, so every
    layer keeps validation rows of its own; a group of one row goes to
    training only rather than into both halves.
    """
    if stratify_by and rows and all(isinstance(r, dict) for r in rows):
        groups: dict[Any, list[Any]] = {}
        for row in rows:
            groups.setdefault(row.get(stratify_by), []).append(row)
        if len(groups) > 1:
            train_rows: list[Any] = []
            val_rows: list[Any] = []
            for key in sorted(groups, key=str):
                if len(groups[key]) == 1:
                    # One row cannot be both trained on and held out honestly.
                    train_rows.extend(groups[key])
                    continue
                t, v = split_dataset(groups[key], val_ratio=val_ratio, seed=seed, stratify_by=None)
                train_rows.extend(t)
                val_rows.extend(v)
            return train_rows, val_rows

    if not rows:
        return [], []
    if len(rows) == 1:
        return list(rows), list(rows)

    shuffled = list(rows)
    rng = random.Random(seed)
    rng.shuffle(shuffled)

    val_count = max(1, int(round(len(shuffled) * val_ratio)))
    val_rows = shuffled[:val_count]
    train_rows = shuffled[val_count:]
    if not train_rows:
        train_rows = val_rows
    return train_rows, val_rows
