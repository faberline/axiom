"""CPU-only tests for the staged material importer."""
import json
import shutil
from pathlib import Path

import pytest

from som.python_material_inventory import PythonMaterialInventoryError, build_staged_material_inventory


def _pack(root: Path, *, bad: bool = False) -> Path:
    source = next(Path("data/som/python-v1/staged-materials/core-python").glob("*/family.json"))
    pack = root / "pack"
    shutil.copytree(source.parent, pack / source.parent.name)
    family = pack / source.parent.name
    if bad:
        metadata = json.loads((family / "family.json").read_text())
        metadata["candidates"] = metadata["candidates"][:6]
        (family / "family.json").write_text(json.dumps(metadata), encoding="utf-8")
    return pack


def test_inventory_is_staged_and_does_not_make_formal_rows(tmp_path: Path) -> None:
    result = build_staged_material_inventory([_pack(tmp_path)])
    assert result["status"] == "staged_not_formal"
    assert result["training_authorized"] is False
    assert result["formal_split_plan"] is None
    assert len(result["families"]) == 1
    assert all("source_path" in item for item in result["families"][0]["candidates"])


def test_failing_quality_pack_is_refused(tmp_path: Path) -> None:
    with pytest.raises(PythonMaterialInventoryError, match="quality audit failed"):
        build_staged_material_inventory([_pack(tmp_path, bad=True)])


def test_existing_packs_can_be_staged() -> None:
    root = Path("data/som/python-v1/staged-materials")
    result = build_staged_material_inventory([root / "core-python", root / "fastapi-pydantic"])
    assert result["status"] == "staged_not_formal"
    assert len(result["families"]) == 32
    assert all(len(item["candidates"]) == 7 for item in result["families"])
