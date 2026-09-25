#!/usr/bin/env python3
"""Tests for the fleet renderer: the tree matches its templates, and every
divergence the renderer is meant to catch is actually caught."""

from __future__ import annotations

import importlib.util
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).with_name("render_fleet.py")
SPEC = importlib.util.spec_from_file_location("render_fleet", SCRIPT)
assert SPEC and SPEC.loader
render_fleet = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = render_fleet
SPEC.loader.exec_module(render_fleet)

REPO = Path(__file__).resolve().parents[2]
OWNED_DIRS = (
    ".claude/agents",
    ".codex/agents",
    "scripts/agents/templates",
    "scripts/agents/singletons",
)


def copy_tree() -> Path:
    tmp = Path(tempfile.mkdtemp(prefix="render-fleet-"))
    for rel in OWNED_DIRS:
        shutil.copytree(REPO / rel, tmp / rel)
    return tmp


def run_cli(root: Path, *flags: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        [sys.executable, str(SCRIPT), "--root", str(root), *flags],
        text=True,
        capture_output=True,
        check=False,
    )


class RenderFleetTests(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = copy_tree()
        self.addCleanup(shutil.rmtree, self.tmp, ignore_errors=True)

    def render_candidate(self) -> None:
        """Render only a disposable candidate tree, never the live fleet."""
        render_fleet.write(self.tmp)
        self.assertEqual(render_fleet.check(self.tmp), [])

    # -- active Codex source and disposable full candidate ------------------

    def test_repo_codex_tree_matches_templates(self) -> None:
        self.assertEqual(render_fleet.check_codex(REPO), [])

    def test_cli_check_codex_passes_on_repo(self) -> None:
        result = run_cli(REPO, "--check-codex")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Codex fleet matches", result.stdout)

    def test_disposable_candidate_matches_templates(self) -> None:
        self.render_candidate()

    def test_rendered_population(self) -> None:
        rendered = render_fleet.rendered_agents(REPO)
        self.assertEqual(len(rendered), 91)
        self.assertNotIn("aw-dev", rendered)
        self.assertIn("aw-pm", rendered)
        self.assertIn("aw-tl", rendered)
        self.assertIn("aw-qa", rendered)
        self.assertIn("sift-dev", rendered)
        self.assertIn("tape-tl", rendered)
        self.assertIn("tape-qa", rendered)
        self.assertIn("tape-pm", rendered)
        # lumen, rig, and the shared libraries moved to faberline repositories.
        self.assertNotIn("lumen-dev", rendered)
        self.assertNotIn("rig-dev", rendered)
        self.assertNotIn("build-stamp-dev", rendered)

    def test_singletons_are_projected_not_rewritten(self) -> None:
        expected = render_fleet.expected_codex_files(REPO)
        codex = render_fleet.codex_agents_dir(REPO)
        for singleton in ("aw-dev", "cto", "project-manager", "tech-design",
                          "integration-qa"):
            self.assertIn(codex / f"{singleton}.toml", expected)

    def test_retired_operator_roles_are_absent(self) -> None:
        expected = render_fleet.expected_codex_files(REPO)
        for name in ("agy" + "-operator", "gke" + "-operator"):
            with self.subTest(name=name):
                self.assertFalse((REPO / ".claude/agents" / f"{name}.md").exists())
                self.assertFalse((REPO / ".codex/agents" / f"{name}.toml").exists())
                self.assertNotIn(REPO / ".codex/agents" / f"{name}.toml", expected)

    def test_codex_write_preserves_claude_agents(self) -> None:
        before = {
            path.relative_to(self.tmp): path.read_bytes()
            for path in sorted((self.tmp / ".claude/agents").glob("*.md"))
        }
        render_fleet.write_codex(self.tmp)
        after = {
            path.relative_to(self.tmp): path.read_bytes()
            for path in sorted((self.tmp / ".claude/agents").glob("*.md"))
        }
        self.assertEqual(after, before)
        self.assertEqual(render_fleet.check_codex(self.tmp), [])

    def test_write_on_a_rendered_candidate_changes_nothing(self) -> None:
        self.render_candidate()
        before = {
            path.relative_to(self.tmp): path.read_bytes()
            for rel in OWNED_DIRS
            for path in sorted((self.tmp / rel).rglob("*"))
            if path.is_file()
        }
        self.assertEqual(render_fleet.write(self.tmp), [])
        after = {
            path.relative_to(self.tmp): path.read_bytes()
            for rel in OWNED_DIRS
            for path in sorted((self.tmp / rel).rglob("*"))
            if path.is_file()
        }
        self.assertEqual(after, before)

    # -- projection shape ----------------------------------------------------

    def test_projection_escapes_description_and_keeps_body_raw(self) -> None:
        markdown = (
            "---\nname: demo-dev\ndescription: Demo — with \"quotes\"\n"
            "model: sonnet\neffort: medium\n---\n\nYou are **demo-dev** — body.\n"
        )
        toml = render_fleet.toml_projection(markdown, "demo-dev.md")
        self.assertIn('description = "Demo \\u2014 with \\"quotes\\""', toml)
        self.assertIn('model_reasoning_effort = "medium"', toml)
        self.assertIn('model = "gpt-5.6-luna"', toml)
        self.assertIn('sandbox_mode = "workspace-write"', toml)
        self.assertIn('nickname_candidates = ["demo-dev", "demo_dev"]', toml)
        self.assertTrue(toml.endswith("'''\nYou are **demo-dev** — body.\n'''\n"))

    def test_projection_refuses_missing_effort(self) -> None:
        markdown = "---\nname: demo-dev\ndescription: Demo\n---\n\nbody\n"
        with self.assertRaisesRegex(render_fleet.RenderError, "no effort"):
            render_fleet.toml_projection(markdown, "demo-dev.md")

    def test_role_model_mapping(self) -> None:
        self.assertEqual(render_fleet.codex_model("tape-pm"), "gpt-5.6-terra")
        self.assertEqual(render_fleet.codex_model("tape-tl"), "gpt-5.6-terra")
        self.assertEqual(render_fleet.codex_model("tape-qa"), "gpt-5.6-luna")
        self.assertEqual(render_fleet.codex_model("tape-dev"), "gpt-5.6-luna")
        self.assertEqual(render_fleet.codex_model("integration-qa"), "gpt-5.6-terra")
        self.assertEqual(render_fleet.codex_sandbox("tape-dev"), "workspace-write")
        self.assertEqual(render_fleet.codex_sandbox("tape-pm"), "read-only")
        self.assertEqual(render_fleet.codex_sandbox("integration-qa"), "read-only")

    def test_qa_and_dev_are_luna_low_worktree_executors(self) -> None:
        rendered = render_fleet.rendered_agents(REPO)
        for role, markdown in rendered.items():
            if not role.endswith(("-qa", "-dev")):
                continue
            with self.subTest(role=role):
                projection = (REPO / ".codex/agents" / f"{role}.toml").read_text()
                self.assertEqual(render_fleet.codex_model(role), "gpt-5.6-luna")
                self.assertIn('model_reasoning_effort = "low"', projection)
                self.assertIn("worktree executor", markdown)
                self.assertIn("scripts/execute_assignment.py", markdown)
                self.assertIn("absolute assignment JSON path", markdown)
                self.assertNotIn("AGY", markdown)
                self.assertNotIn("model=", markdown)
                self.assertNotIn("effort=", markdown)
                self.assertNotIn("agy" + "-operator", markdown)
                self.assertNotIn("gke" + "-operator", markdown)

        aw = (REPO / ".claude/agents/aw-dev.md").read_text()
        aw_projection = (REPO / ".codex/agents/aw-dev.toml").read_text()
        self.assertIn("worktree executor", aw)
        self.assertIn("scripts/execute_assignment.py", aw)
        self.assertNotIn("AGY", aw)
        self.assertIn('model = "gpt-5.6-luna"', aw_projection)
        self.assertIn('model_reasoning_effort = "low"', aw_projection)

    def test_codex_fleet_readme_keeps_executor_backend_neutral(self) -> None:
        readme = (REPO / ".codex/agents/README.md").read_text()
        self.assertIn("scripts/execute_assignment.py", readme)
        retired_operator_names = (
            "agy" + "-operator",
            "gke" + "-operator",
        )
        for forbidden in (
            "AGY",
            "Antigravity",
            "dispatch-to-agy",
            "gemini-3.8-flash",
            *retired_operator_names,
        ):
            with self.subTest(forbidden=forbidden):
                self.assertNotIn(forbidden, readme)

    # -- negative controls: each divergence class is caught ------------------

    def test_template_mutation_is_caught(self) -> None:
        self.render_candidate()
        template = self.tmp / "scripts/agents/templates/app/dev.md"
        template.write_text(
            template.read_text(encoding="utf-8").replace("## Goal", "## Goal!", 1),
            encoding="utf-8",
        )
        findings = render_fleet.check(self.tmp)
        self.assertIn("differs: .claude/agents/tape-dev.md", findings)
        self.assertIn("differs: .codex/agents/tape-dev.toml", findings)
        self.assertNotIn("differs: .claude/agents/sift-pm.md", findings)
        result = run_cli(self.tmp, "--check")
        self.assertEqual(result.returncode, 1, result.stdout)

    def test_hand_edited_rendered_file_is_caught(self) -> None:
        self.render_candidate()
        path = self.tmp / ".claude/agents/tape-dev.md"
        path.write_text(path.read_text(encoding="utf-8") + "\n- extra\n",
                        encoding="utf-8")
        self.assertEqual(render_fleet.check(self.tmp),
                         ["differs: .claude/agents/tape-dev.md"])

    def test_stale_projection_is_caught_and_removed(self) -> None:
        self.render_candidate()
        (self.tmp / ".codex/agents/ghost-dev.toml").write_text(
            'name = "ghost-dev"\nmodel_reasoning_effort = "max"\n', encoding="utf-8"
        )
        self.assertEqual(render_fleet.check(self.tmp),
                         ["stray projection: .codex/agents/ghost-dev.toml"])
        self.assertEqual(render_fleet.write(self.tmp),
                         ["removed: .codex/agents/ghost-dev.toml"])
        self.assertEqual(render_fleet.check(self.tmp), [])

    def test_missing_projection_is_caught_and_written(self) -> None:
        self.render_candidate()
        (self.tmp / ".codex/agents/cap-qa.toml").unlink()
        self.assertEqual(render_fleet.check(self.tmp),
                         ["missing: .codex/agents/cap-qa.toml"])
        self.assertEqual(render_fleet.write(self.tmp),
                         ["wrote: .codex/agents/cap-qa.toml"])
        self.assertEqual(render_fleet.check(self.tmp), [])

    def test_singleton_edit_reprojects_only_its_toml(self) -> None:
        self.render_candidate()
        path = self.tmp / ".claude/agents/tech-design.md"
        path.write_text(path.read_text(encoding="utf-8") + "\n- extra\n",
                        encoding="utf-8")
        self.assertEqual(render_fleet.check(self.tmp),
                         ["differs: .codex/agents/tech-design.toml"])
        self.assertEqual(render_fleet.write(self.tmp),
                         ["wrote: .codex/agents/tech-design.toml"])
        self.assertTrue(path.read_text(encoding="utf-8").endswith("- extra\n"))


if __name__ == "__main__":
    unittest.main()
