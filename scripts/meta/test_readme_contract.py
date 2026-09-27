#!/usr/bin/env python3
"""Tests for the project README contract validator."""

from __future__ import annotations

import importlib.util
import sys
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).with_name("readme_contract.py")
SPEC = importlib.util.spec_from_file_location("readme_contract", SCRIPT)
assert SPEC and SPEC.loader
readme_contract = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = readme_contract
SPEC.loader.exec_module(readme_contract)


VALID_README = """# Demo

## Brief

Demo indexes caller-owned values and returns caller-owned IDs.

## Primary workflow

1. Declare a schema.
2. Index a value.
3. Query the index.

## Public operations

Use `PUT /items` and `POST /items/search`.

## Contract discovery

Run `demo spec` for the wire contract.

## Capabilities

Every entry is a product capability.

### Capability index

| Capability | ID | User promise | Sources |
|---|---|---|---|
| Search | `search` | Return matching caller IDs. | `apps/demo`, `libs/shared`, `external:runtime` |

### Search

- ID: `search`
- Promise: Return matching caller IDs.
- Sources:
  - [`apps/demo`](./) defines query behavior and product composition.
  - [`libs/shared`](../../libs/shared/README.md) provides reusable index mechanics.
  - `external:runtime` runs the declared service contract.
- Gate: `scripts/check-demo.sh`

## Supporting documents

- [Contributing](CONTRIBUTING.md)
"""


class ReadmeContractTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.repo = Path(self.temp.name)
        (self.repo / "apps/demo").mkdir(parents=True)
        (self.repo / "libs/shared").mkdir(parents=True)
        (self.repo / "scripts").mkdir()
        (self.repo / "apps/demo/README.md").write_text(VALID_README, encoding="utf-8")
        (self.repo / "apps/demo/CONTRIBUTING.md").write_text("# Demo contributing\n", encoding="utf-8")
        (self.repo / "libs/shared/README.md").write_text("# Shared\n", encoding="utf-8")
        (self.repo / "scripts/check-demo.sh").write_text("#!/bin/sh\n", encoding="utf-8")
        self.readme = self.repo / "apps/demo/README.md"

    def tearDown(self) -> None:
        self.temp.cleanup()

    def validate(
        self,
        text: str | None = None,
        package_index: dict[str, set[str]] | None = None,
    ):
        if text is not None:
            self.readme.write_text(text, encoding="utf-8")
        return readme_contract.validate_readme(
            self.repo,
            self.readme,
            package_index if package_index is not None else {},
        )

    def rules(self, report) -> set[str]:
        return {finding.rule for finding in report.findings}

    def test_valid_contract_passes(self) -> None:
        report = self.validate()
        self.assertTrue(report.ok, report.as_dict())
        self.assertEqual(report.product_sections, ["Public operations"])
        self.assertEqual([capability.capability_id for capability in report.capabilities], ["search"])
        self.assertEqual(report.gate_count, 1)

    def test_requires_a_product_specific_functional_section(self) -> None:
        text = VALID_README.replace(
            "## Public operations\n\nUse `PUT /items` and `POST /items/search`.\n\n",
            "",
        )
        report = self.validate(text)
        self.assertIn("R4", self.rules(report))

    def test_index_and_detail_sources_must_match(self) -> None:
        text = VALID_README.replace(
            "`apps/demo`, `libs/shared`, `external:runtime`",
            "`apps/demo`, `libs/shared`",
        )
        report = self.validate(text)
        self.assertIn("R7", self.rules(report))

    def test_rejects_old_hierarchy_status_and_work_item_fields(self) -> None:
        text = VALID_README.replace(
            "### Search\n\n- ID: `search`",
            "### Core Features\n\n#### Search\n\n- Status: ready\n- Root WI: #123\n- ID: `search`",
        )
        report = self.validate(text)
        rules = self.rules(report)
        self.assertIn("R6", rules)
        self.assertIn("R10", rules)
        self.assertIn("R12", rules)

    def test_broken_links_and_missing_gate_scripts_fail(self) -> None:
        text = VALID_README.replace("CONTRIBUTING.md", "missing.md").replace(
            "scripts/check-demo.sh", "scripts/missing.sh"
        )
        report = self.validate(text)
        self.assertIn("R8", self.rules(report))
        self.assertIn("R9", self.rules(report))

    def test_cargo_gate_requires_real_package_and_target(self) -> None:
        text = VALID_README.replace(
            "scripts/check-demo.sh",
            "cargo test -p demo --test search_e2e",
        )
        good = self.validate(text, {"demo": {"search_e2e"}})
        self.assertTrue(good.ok, good.as_dict())

        bad = self.validate(text, {"demo": {"other"}})
        self.assertIn("R8", self.rules(bad))

    def test_cargo_gate_rejects_bare_name_filters(self) -> None:
        text = VALID_README.replace(
            "scripts/check-demo.sh",
            "cargo test -p demo search_behavior",
        )
        report = self.validate(text, {"demo": {"search_e2e"}})
        self.assertIn("R8", self.rules(report))

    def test_heading_and_status_examples_inside_fences_are_not_structure(self) -> None:
        text = VALID_README.replace(
            "Use `PUT /items` and `POST /items/search`.",
            """Use `PUT /items` and `POST /items/search`.

```markdown
## Capabilities
#### Legacy example
Status: ready
```""",
        ).replace(
            "- Gate: `scripts/check-demo.sh`",
            """- Gate: `scripts/check-demo.sh`

```yaml
Status: ready
```""",
        )
        report = self.validate(text)
        self.assertTrue(report.ok, report.as_dict())

    def test_prompt_binds_the_review_to_current_bytes(self) -> None:
        report = self.validate()
        prompt = readme_contract.clean_reader_prompt(report, self.readme)
        self.assertIn(report.sha256, prompt)
        self.assertIn(str(self.readme), prompt)
        self.assertIn("Return JSON only", prompt)
        self.assertNotIn("Return matching caller IDs", prompt)

    def test_models_source_is_accepted_when_its_directory_exists(self) -> None:
        (self.repo / "models/demo").mkdir(parents=True)
        (self.repo / "models/demo/README.md").write_text("# Model demo\n", encoding="utf-8")
        text = VALID_README.replace(
            "`apps/demo`, `libs/shared`, `external:runtime`",
            "`apps/demo`, `models/demo`, `external:runtime`",
        ).replace(
            "  - [`libs/shared`](../../libs/shared/README.md) provides reusable index mechanics.",
            "  - [`models/demo`](../../models/demo/README.md) provides the trained ranking head.",
        )
        report = self.validate(text)
        self.assertTrue(report.ok, report.as_dict())
        self.assertEqual([capability.sources for capability in report.capabilities],
                         [["apps/demo", "models/demo", "external:runtime"]])

    def test_models_source_must_exist_on_disk(self) -> None:
        text = VALID_README.replace(
            "`apps/demo`, `libs/shared`, `external:runtime`",
            "`apps/demo`, `models/missing`, `external:runtime`",
        ).replace(
            "  - [`libs/shared`](../../libs/shared/README.md) provides reusable index mechanics.",
            "  - `models/missing` provides the trained ranking head.",
        )
        report = self.validate(text)
        self.assertIn("R7", self.rules(report))


class RuntimeSkillStaysDeletedTest(unittest.TestCase):
    def test_no_skill_mirror_reappears(self) -> None:
        # The project-readme-check skill pair was deleted on 2026-09-02; the
        # validator script is the whole check now. A resurrected mirror would
        # silently outrank it again.
        repo = Path(__file__).resolve().parents[2]
        for root in (".claude/skills", ".agents/skills"):
            path = repo / root / "project-readme-check"
            self.assertFalse(path.exists(), path)


if __name__ == "__main__":
    unittest.main()
