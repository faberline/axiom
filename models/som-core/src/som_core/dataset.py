"""Dataset parser for Structured Outcome Models (SOM).

Parses oracle-family corpora (``materials/<family>/family.json`` plus one
candidate file per entry) and compiled JSONL records for MLX training and
evaluation.

Rules this module owns:

- A corpus is one directory whose ``materials/`` holds the families; several
  corpora (``som-code``, ``som-code-python``, ``som-code-rust``) are loaded
  together by :func:`load_corpora`, which refuses an empty corpus and a
  family id that two corpora both claim, so a union is never silently a
  subset and a row id is never ambiguous.
- The curation fields a corpus project measures for each near miss,
  ``why_wrong`` and ``caught_by``, ride on :class:`CandidateRecord` unchanged
  and reach the training row through :meth:`CandidateRecord.to_dict`; a gold
  candidate carries neither. The engine never rewrites them.
- Family-level curation (``rationale``, ``oracle``) reaches the row through
  ``metadata`` with every other ``family.json`` key that is not a candidate,
  the requirement, or the skeleton.
"""

from __future__ import annotations

from dataclasses import dataclass, field
import json
import os
from pathlib import Path
import random
from typing import Any, Iterable, Optional


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
    state: str = ""
    question: str = ""
    metadata: dict[str, Any] = field(default_factory=dict)

    def __post_init__(self) -> None:
        if not self.state:
            self.state = (
                f"Domain: {self.domain} | Area: {self.area} | Capability: {self.capability}\n"
                f"Requirement: {self.requirement}\n"
                f"Fixed executable skeleton:\n{self.skeleton}"
            )
        if not self.question:
            self.question = "Which candidate implementation correctly satisfies all specifications without defects?"

    def to_decision_dict(self) -> dict[str, Any]:
        """Convert to dictionary matching SOM training and DecisionRequest schema."""
        return {
            "id": f"som:{self.domain}:{self.family_id}",
            "family": self.family_id,
            "domain": self.domain,
            "area": self.area,
            "capability": self.capability,
            "state": self.state,
            "question": self.question,
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


def load_materials_dataset(materials_dir: Path | str) -> list[ScenarioFamily]:
    """Scan a materials directory and parse all scenario families.

    Parameters
    ----------
    materials_dir : Path | str
        Path to materials directory (e.g. data/python-v2/materials).

    Returns
    -------
    list[ScenarioFamily]
        Sorted list of parsed scenario families.
    """
    path = Path(materials_dir).resolve()
    if not path.is_dir():
        raise NotADirectoryError(f"Materials directory does not exist: {path}")

    family_dirs = [d for d in path.iterdir() if d.is_dir() and (d / "family.json").is_file()]
    if not family_dirs:
        family_dirs = [fj.parent for fj in path.rglob("family.json")]

    family_dirs = sorted(set(family_dirs), key=lambda d: d.name)
    scenarios: list[ScenarioFamily] = []
    for fdir in family_dirs:
        scenarios.append(load_family_from_metadata(fdir))

    return scenarios


def load_jsonl_dataset(jsonl_path: Path | str) -> list[dict[str, Any]]:
    """Parse rows from a JSONL dataset file.

    Parameters
    ----------
    jsonl_path : Path | str
        Path to .jsonl file.

    Returns
    -------
    list[dict[str, Any]]
        List of parsed example dictionaries.
    """
    path = Path(jsonl_path).resolve()
    if not path.is_file():
        raise FileNotFoundError(f"JSONL file does not exist: {path}")

    rows: list[dict[str, Any]] = []
    with path.open("r", encoding="utf-8") as f:
        for idx, line in enumerate(f, start=1):
            line = line.strip()
            if not line:
                continue
            row = json.loads(line)
            if "candidates" not in row or "gold_candidate_id" not in row:
                raise ValueError(f"Line {idx} in {path} is missing required SOM keys ('candidates', 'gold_candidate_id')")
            rows.append(row)

    return rows


def export_dataset_to_jsonl(
    dataset: list[ScenarioFamily] | list[dict[str, Any]],
    output_path: Path | str,
) -> Path:
    """Export a dataset (either ScenarioFamily objects or dicts) to JSONL format."""
    path = Path(output_path).resolve()
    path.parent.mkdir(parents=True, exist_ok=True)

    with path.open("w", encoding="utf-8") as f:
        for item in dataset:
            row = item.to_decision_dict() if isinstance(item, ScenarioFamily) else item
            f.write(json.dumps(row, ensure_ascii=False) + "\n")

    return path


def find_python_v2_dir(search_root: Path | str | None = None) -> Path:
    """Locate the som-code-python/data/python-v2 directory."""
    env_path = os.environ.get("SOM_PYTHON_V2_DATA")
    if env_path and Path(env_path).exists():
        return Path(env_path).resolve()

    start = Path(search_root).resolve() if search_root else Path.cwd().resolve()
    candidates = [
        start / "models" / "som-code-python" / "data" / "python-v2",
        start / "som-code-python" / "data" / "python-v2",
        start / "data" / "python-v2",
        start.parent / "som-code-python" / "data" / "python-v2",
        start.parent.parent / "som-code-python" / "data" / "python-v2",
        start.parents[2] / "som-code-python" / "data" / "python-v2" if len(start.parents) > 2 else None,
        Path(__file__).resolve().parents[4] / "models" / "som-code-python" / "data" / "python-v2",
    ]

    for cand in candidates:
        if cand and cand.exists() and cand.is_dir():
            return cand.resolve()

    raise FileNotFoundError(
        "Could not automatically locate 'som-code-python/data/python-v2'. "
        "Please provide the path explicitly or set SOM_PYTHON_V2_DATA."
    )


def load_python_v2_dataset(
    data_dir: Path | str | None = None,
    as_dicts: bool = True,
) -> list[dict[str, Any]] | list[ScenarioFamily]:
    """High-level loader for python-v2 dataset."""
    base_path = Path(data_dir).resolve() if data_dir else find_python_v2_dir()

    if base_path.is_file() and base_path.suffix == ".jsonl":
        rows = load_jsonl_dataset(base_path)
        return rows

    materials_dir = base_path / "materials" if (base_path / "materials").is_dir() else base_path
    if (materials_dir / "materials").is_dir():
        materials_dir = materials_dir / "materials"

    scenarios = load_materials_dataset(materials_dir)
    if as_dicts:
        return [s.to_decision_dict() for s in scenarios]
    return scenarios


def load_corpora(
    data_dirs: Iterable[Path | str] | None = None,
    as_dicts: bool = True,
) -> list[dict[str, Any]] | list[ScenarioFamily]:
    """Load one or more corpora into a single dataset, in the order given.

    ``None`` or an empty iterable falls back to the auto-discovered python-v2
    corpus, the same as :func:`load_python_v2_dataset` with no argument. A
    corpus that yields no family, and a row id that two corpora both produce,
    are refused with ``ValueError`` naming the directories, so a training run
    over three corpora is never silently a run over two.
    """
    dirs = [Path(d) for d in (data_dirs or [])]
    if not dirs:
        return load_python_v2_dataset(data_dir=None, as_dicts=as_dicts)

    rows: list[Any] = []
    seen: dict[str, Path] = {}
    for corpus in dirs:
        loaded = load_python_v2_dataset(data_dir=corpus, as_dicts=as_dicts)
        if not loaded:
            raise ValueError(f"Corpus yields no family: {corpus}")
        for item in loaded:
            row_id = item["id"] if as_dicts else item.to_decision_dict()["id"]
            if row_id in seen:
                raise ValueError(
                    f"Family id {row_id!r} is claimed by both {seen[row_id]} and {corpus}"
                )
            seen[row_id] = corpus
        rows.extend(loaded)
    return rows


def split_dataset(
    rows: list[Any],
    val_ratio: float = 0.25,
    seed: int = 42,
) -> tuple[list[Any], list[Any]]:
    """Deterministically split dataset into (train_rows, val_rows)."""
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
