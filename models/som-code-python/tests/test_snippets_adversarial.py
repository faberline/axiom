#!/usr/bin/env python3
"""Adversarial mutation test suite for SOM Code Snippet Library.

Verifies that scripts/verify_snippets.py robustly catches and rejects:
1. Malformed JSON syntax
2. Missing required schema keys (id, description, imports, template)
3. Schema type violations (imports not list of str, template empty or non-str, root not object)
4. Mismatched snippet IDs
5. Broken Python syntax in templates (mismatched parentheses, invalid indentation, illegal tokens)
6. Broken Python syntax in imports statements
7. Missing snippet files or categories
8. Unmapped template placeholders
9. Baseline clean suite compiles cleanly (exit code 0)
10. Production snippets remain completely untouched and identical to baseline sha256
"""

from __future__ import annotations

import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from typing import Callable, Tuple

REPO_ROOT = Path(__file__).resolve().parent.parent
REAL_SCRIPT = REPO_ROOT / "scripts" / "verify_snippets.py"
REAL_SNIPPETS_DIR = REPO_ROOT / "data" / "snippets"

BASELINE_SHA256 = {
    "fastapi/fastapi_init.json": "12329225cbb677a3df555e95e82c191c735785c0ec58c65905da380a94cb29ee",
    "fastapi/fastapi_route_get.json": "f9d845097e17550592ff5b6ef04a94f135f4c6f31d9de4ab3fb5dfdfca9309e6",
    "fastapi/fastapi_route_post.json": "9c0cf1604eb2a8e06eae877c669b8312317b21cd61849d01d2b8f553dabcce64",
    "fastapi/fastapi_router.json": "e959761da84c6f280ee0670eebaf4c9d6354a9d3f00b545132898b7779952604",
    "pydantic/pydantic_base.json": "8b3b4786a3002b44425a25e41929fecc9da6f43fad3b0cdd9d58f23279bd11bb",
    "pydantic/pydantic_field.json": "71b48ba6cfbfd0d63936f9607af2a3a6201ee4067b5d04a36b02dc771346d77b",
    "sqlalchemy/sqlalchemy_async_engine.json": "f2fc8f7ea5d9ca9d379363aab17a7c8f36479b0e842b24d94c057c1b7be590c6",
    "sqlalchemy/sqlalchemy_model.json": "4efe27fbf1694bdadd25bb5916da1d7550cf1fb284f1cffaa578ef9e95fa67de",
    "scripts/verify_snippets.py": "493532f0b907a4ceae21fc370bb3b9cad5b6468af476561445d1e8c435b5ebad",
}


class TestSnippetsAdversarial(unittest.TestCase):
    def run_in_temp_harness(
        self, mutate_fn: Callable[[Path], None] | None = None
    ) -> Tuple[int, str, str]:
        with tempfile.TemporaryDirectory() as td:
            temp_path = Path(td)
            (temp_path / "scripts").mkdir(parents=True)
            (temp_path / "data").mkdir(parents=True)

            shutil.copy(REAL_SCRIPT, temp_path / "scripts" / "verify_snippets.py")
            shutil.copytree(
                REAL_SNIPPETS_DIR, temp_path / "data" / "snippets"
            )

            if mutate_fn is not None:
                mutate_fn(temp_path)

            res = subprocess.run(
                [sys.executable, str(temp_path / "scripts" / "verify_snippets.py")],
                cwd=temp_path,
                capture_output=True,
                text=True,
            )
            return res.returncode, res.stdout, res.stderr

    def test_01_baseline_clean_suite(self):
        """Clean snippet suite must pass with exit code 0."""
        rc, stdout, stderr = self.run_in_temp_harness(None)
        self.assertEqual(rc, 0, f"Baseline failed: {stderr}")
        self.assertIn("[SUCCESS]", stdout)
        self.assertIn("All 8 snippets validated, assembled, and successfully compiled!", stdout)

    def test_02_corrupted_json_syntax(self):
        """Malformed JSON syntax must trigger exit code 1 and JSONDecodeError."""
        def mutate(tp: Path):
            target = tp / "data" / "snippets" / "fastapi" / "fastapi_init.json"
            target.write_text('{"id": "fastapi_init", broken_json: true', encoding="utf-8")

        rc, stdout, stderr = self.run_in_temp_harness(mutate)
        self.assertEqual(rc, 1)
        self.assertTrue("JSONDecodeError" in stderr or "Expecting" in stderr, stderr)

    def test_03_missing_required_key_template(self):
        """Missing 'template' key must trigger exit code 1."""
        def mutate(tp: Path):
            target = tp / "data" / "snippets" / "fastapi" / "fastapi_router.json"
            data = json.loads(target.read_text(encoding="utf-8"))
            del data["template"]
            target.write_text(json.dumps(data), encoding="utf-8")

        rc, stdout, stderr = self.run_in_temp_harness(mutate)
        self.assertEqual(rc, 1)
        self.assertIn("Missing required field 'template'", stderr)

    def test_04_missing_required_key_id(self):
        """Missing 'id' key must trigger exit code 1."""
        def mutate(tp: Path):
            target = tp / "data" / "snippets" / "fastapi" / "fastapi_route_post.json"
            data = json.loads(target.read_text(encoding="utf-8"))
            del data["id"]
            target.write_text(json.dumps(data), encoding="utf-8")

        rc, stdout, stderr = self.run_in_temp_harness(mutate)
        self.assertEqual(rc, 1)
        self.assertIn("Missing required field 'id'", stderr)

    def test_05_missing_required_key_imports(self):
        """Missing 'imports' key must trigger exit code 1."""
        def mutate(tp: Path):
            target = tp / "data" / "snippets" / "pydantic" / "pydantic_base.json"
            data = json.loads(target.read_text(encoding="utf-8"))
            del data["imports"]
            target.write_text(json.dumps(data), encoding="utf-8")

        rc, stdout, stderr = self.run_in_temp_harness(mutate)
        self.assertEqual(rc, 1)
        self.assertIn("Missing required field 'imports'", stderr)

    def test_06_missing_required_key_description(self):
        """Missing 'description' key must trigger exit code 1."""
        def mutate(tp: Path):
            target = tp / "data" / "snippets" / "sqlalchemy" / "sqlalchemy_model.json"
            data = json.loads(target.read_text(encoding="utf-8"))
            del data["description"]
            target.write_text(json.dumps(data), encoding="utf-8")

        rc, stdout, stderr = self.run_in_temp_harness(mutate)
        self.assertEqual(rc, 1)
        self.assertIn("Missing required field 'description'", stderr)

    def test_07_broken_python_syntax_mismatched_parentheses(self):
        """Unclosed parenthesis in template must fail AST/py_compile with exit code 1."""
        def mutate(tp: Path):
            target = tp / "data" / "snippets" / "fastapi" / "fastapi_init.json"
            data = json.loads(target.read_text(encoding="utf-8"))
            data["template"] = '{{app_name}} = FastAPI(\n    title="{{title}}"'
            target.write_text(json.dumps(data), encoding="utf-8")

        rc, stdout, stderr = self.run_in_temp_harness(mutate)
        self.assertEqual(rc, 1)
        self.assertTrue("never closed" in stderr or "SyntaxError" in stderr or "unexpected EOF" in stderr, stderr)

    def test_08_broken_python_syntax_indentation_error(self):
        """Indentation error in template must fail AST/py_compile with exit code 1."""
        def mutate(tp: Path):
            target = tp / "data" / "snippets" / "pydantic" / "pydantic_field.json"
            data = json.loads(target.read_text(encoding="utf-8"))
            data["template"] = '   {{field_name}}: {{field_type}} = Field()'
            target.write_text(json.dumps(data), encoding="utf-8")

        rc, stdout, stderr = self.run_in_temp_harness(mutate)
        self.assertEqual(rc, 1)
        self.assertTrue("unindent" in stderr or "indent" in stderr.lower(), stderr)

    def test_09_broken_python_syntax_invalid_tokens(self):
        """Illegal syntax tokens in template must fail AST/py_compile with exit code 1."""
        def mutate(tp: Path):
            target = tp / "data" / "snippets" / "sqlalchemy" / "sqlalchemy_model.json"
            data = json.loads(target.read_text(encoding="utf-8"))
            data["template"] = "class {{table_class_name}}(Base): === invalid ==="
            target.write_text(json.dumps(data), encoding="utf-8")

        rc, stdout, stderr = self.run_in_temp_harness(mutate)
        self.assertEqual(rc, 1)
        self.assertTrue("invalid syntax" in stderr or "SyntaxError" in stderr, stderr)

    def test_10_broken_python_syntax_in_imports(self):
        """Malformed import line must fail AST/py_compile with exit code 1."""
        def mutate(tp: Path):
            target = tp / "data" / "snippets" / "fastapi" / "fastapi_route_get.json"
            data = json.loads(target.read_text(encoding="utf-8"))
            data["imports"] = ["from import def =="]
            target.write_text(json.dumps(data), encoding="utf-8")

        rc, stdout, stderr = self.run_in_temp_harness(mutate)
        self.assertEqual(rc, 1)
        self.assertTrue("invalid syntax" in stderr or "SyntaxError" in stderr, stderr)

    def test_11_mismatched_snippet_id(self):
        """Mismatched snippet ID vs expected ID must trigger exit code 1."""
        def mutate(tp: Path):
            target = tp / "data" / "snippets" / "pydantic" / "pydantic_field.json"
            data = json.loads(target.read_text(encoding="utf-8"))
            data["id"] = "wrong_field_id"
            target.write_text(json.dumps(data), encoding="utf-8")

        rc, stdout, stderr = self.run_in_temp_harness(mutate)
        self.assertEqual(rc, 1)
        self.assertIn("does not match expected 'pydantic_field'", stderr)

    def test_12_imports_not_a_list(self):
        """Non-list 'imports' field must trigger exit code 1."""
        def mutate(tp: Path):
            target = tp / "data" / "snippets" / "fastapi" / "fastapi_init.json"
            data = json.loads(target.read_text(encoding="utf-8"))
            data["imports"] = "from fastapi import FastAPI"
            target.write_text(json.dumps(data), encoding="utf-8")

        rc, stdout, stderr = self.run_in_temp_harness(mutate)
        self.assertEqual(rc, 1)
        self.assertIn("'imports' must be a list of strings", stderr)

    def test_13_imports_contains_non_string(self):
        """Non-string item in 'imports' list must trigger exit code 1."""
        def mutate(tp: Path):
            target = tp / "data" / "snippets" / "fastapi" / "fastapi_init.json"
            data = json.loads(target.read_text(encoding="utf-8"))
            data["imports"] = ["from fastapi import FastAPI", 42]
            target.write_text(json.dumps(data), encoding="utf-8")

        rc, stdout, stderr = self.run_in_temp_harness(mutate)
        self.assertEqual(rc, 1)
        self.assertIn("'imports' must be a list of strings", stderr)

    def test_14_empty_template_string(self):
        """Empty or whitespace-only template string must trigger exit code 1."""
        def mutate(tp: Path):
            target = tp / "data" / "snippets" / "fastapi" / "fastapi_router.json"
            data = json.loads(target.read_text(encoding="utf-8"))
            data["template"] = "   \n\t  "
            target.write_text(json.dumps(data), encoding="utf-8")

        rc, stdout, stderr = self.run_in_temp_harness(mutate)
        self.assertEqual(rc, 1)
        self.assertIn("'template' must be a non-empty string", stderr)

    def test_15_non_string_template(self):
        """Non-string template field must trigger exit code 1."""
        def mutate(tp: Path):
            target = tp / "data" / "snippets" / "fastapi" / "fastapi_router.json"
            data = json.loads(target.read_text(encoding="utf-8"))
            data["template"] = 12345
            target.write_text(json.dumps(data), encoding="utf-8")

        rc, stdout, stderr = self.run_in_temp_harness(mutate)
        self.assertEqual(rc, 1)
        self.assertIn("'template' must be a non-empty string", stderr)

    def test_16_root_not_a_json_object(self):
        """JSON root that is not an object must trigger exit code 1."""
        def mutate(tp: Path):
            target = tp / "data" / "snippets" / "pydantic" / "pydantic_base.json"
            target.write_text("[1, 2, 3]", encoding="utf-8")

        rc, stdout, stderr = self.run_in_temp_harness(mutate)
        self.assertEqual(rc, 1)
        self.assertIn("Root must be a JSON object", stderr)

    def test_17_missing_snippet_file(self):
        """Missing snippet file on disk must trigger exit code 1."""
        def mutate(tp: Path):
            target = tp / "data" / "snippets" / "fastapi" / "fastapi_route_get.json"
            os.remove(target)

        rc, stdout, stderr = self.run_in_temp_harness(mutate)
        self.assertEqual(rc, 1)
        self.assertIn("Missing snippet file", stderr)

    def test_18_missing_category_directory(self):
        """Missing category directory must trigger exit code 1."""
        def mutate(tp: Path):
            target = tp / "data" / "snippets" / "sqlalchemy"
            shutil.rmtree(target)

        rc, stdout, stderr = self.run_in_temp_harness(mutate)
        self.assertEqual(rc, 1)
        self.assertIn("Missing category directory", stderr)

    def test_19_unknown_template_placeholder(self):
        """Unmapped template placeholder variable must trigger exit code 1."""
        def mutate(tp: Path):
            target = tp / "data" / "snippets" / "sqlalchemy" / "sqlalchemy_async_engine.json"
            data = json.loads(target.read_text(encoding="utf-8"))
            data["template"] += "\n# {{unknown_dummy_var}}"
            target.write_text(json.dumps(data), encoding="utf-8")

        rc, stdout, stderr = self.run_in_temp_harness(mutate)
        self.assertEqual(rc, 1)
        self.assertIn("Missing dummy substitution for placeholder: '{{unknown_dummy_var}}'", stderr)

    def test_20_production_snippets_integrity(self):
        """Assert all production files in the repo are 100% intact and match baseline sha256."""
        for rel_path, expected_hash in BASELINE_SHA256.items():
            full_path = REPO_ROOT / rel_path if not rel_path.startswith("scripts/") else REPO_ROOT / rel_path
            if rel_path.startswith("scripts/"):
                full_path = REPO_ROOT / rel_path
            else:
                full_path = REPO_ROOT / "data" / "snippets" / rel_path
            self.assertTrue(full_path.is_file(), f"Missing production file: {full_path}")
            current_hash = hashlib.sha256(full_path.read_bytes()).hexdigest()
            self.assertEqual(
                current_hash,
                expected_hash,
                f"Production file {rel_path} was corrupted! Expected {expected_hash}, got {current_hash}",
            )


if __name__ == "__main__":
    unittest.main()
