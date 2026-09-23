#!/usr/bin/env python3
"""SOM Code Snippet Library Verifier (scripts/verify_snippets.py).

Validates:
1. Presence and JSON schema integrity of all 8 required snippet definitions under data/snippets/.
2. Mustache parameter consistency.
3. Assembly of a mock `main.py` combining all 8 snippets in dependency order.
4. Syntactic compilation via `ast.parse` and `python -m py_compile main.py` with exit code 0.
"""

from __future__ import annotations

import ast
import json
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any, Dict, List

REQUIRED_SNIPPETS = {
    "fastapi": ["fastapi_init", "fastapi_router", "fastapi_route_post", "fastapi_route_get"],
    "pydantic": ["pydantic_base", "pydantic_field"],
    "sqlalchemy": ["sqlalchemy_async_engine", "sqlalchemy_model"],
}

ORDERED_SNIPPET_IDS = [
    "sqlalchemy_async_engine",
    "sqlalchemy_model",
    "pydantic_base",
    "pydantic_field",
    "fastapi_init",
    "fastapi_router",
    "fastapi_route_post",
    "fastapi_route_get",
]

DUMMY_VALUES: Dict[str, str] = {
    # fastapi_init
    "app_name": "app",
    "title": "SOM Generated Service",
    "version": "0.1.0",
    "description": "Auto-assembled mock service for verification",
    # fastapi_router
    "router_name": "router",
    "prefix": "/api/v1",
    "tag": "items",
    # fastapi_route_post
    "path": "/items",
    "response_model": "ItemSchema",
    "handler_name": "create_item",
    "request_model": "ItemSchema",
    "docstring": "Create a new item record.",
    # fastapi_route_get
    "param_name": "item_id",
    "param_type": "int",
    "return_expression": 'ItemSchema(tags=["default"])',
    # pydantic_base
    "model_name": "ItemSchema",
    # pydantic_field
    "field_name": "tags",
    "field_type": "list[str]",
    "default_factory": "list",
    # sqlalchemy_async_engine
    "engine_name": "engine",
    "database_url": "sqlite+aiosqlite:///:memory:",
    "echo": "False",
    "session_factory_name": "async_session_factory",
    # sqlalchemy_model
    "table_class_name": "ItemRecord",
    "table_name": "items",
    "column_name": "title",
    "column_type": "str",
    "column_sql_type": "String(length=255)",
}


def find_snippets_dir() -> Path:
    current = Path(__file__).resolve().parent
    candidates = [
        current.parent / "data" / "snippets",
        Path.cwd() / "data" / "snippets",
        Path.cwd() / "models" / "som-code-python" / "data" / "snippets",
    ]
    for c in candidates:
        if c.is_dir():
            return c
    raise FileNotFoundError("Could not locate data/snippets directory.")


def validate_snippet_schema(data: Any, expected_id: str, path: Path) -> None:
    if not isinstance(data, dict):
        raise ValueError(f"{path}: Root must be a JSON object.")
    for key in ("id", "description", "imports", "template"):
        if key not in data:
            raise ValueError(f"{path}: Missing required field '{key}'.")
    if data["id"] != expected_id:
        raise ValueError(f"{path}: ID '{data['id']}' does not match expected '{expected_id}'.")
    if not isinstance(data["imports"], list) or not all(isinstance(i, str) for i in data["imports"]):
        raise ValueError(f"{path}: 'imports' must be a list of strings.")
    if not isinstance(data["template"], str) or not data["template"].strip():
        raise ValueError(f"{path}: 'template' must be a non-empty string.")


def substitute_template(template: str, dummy_values: Dict[str, str]) -> str:
    def replacer(match: re.Match[str]) -> str:
        var_name = match.group(1)
        if var_name not in dummy_values:
            raise KeyError(f"Missing dummy substitution for placeholder: '{{{{{var_name}}}}}'")
        return dummy_values[var_name]

    return re.sub(r"\{\{([a-zA-Z0-9_]+)\}\}", replacer, template)


def assemble_mock_main(snippets_by_id: Dict[str, Dict[str, Any]]) -> str:
    # 1. Deduplicate imports
    imports_set = set()
    imports_list: List[str] = []
    for snippet_id in ORDERED_SNIPPET_IDS:
        snippet = snippets_by_id[snippet_id]
        for imp in snippet.get("imports", []):
            imp_clean = imp.strip()
            if imp_clean and imp_clean not in imports_set:
                imports_set.add(imp_clean)
                imports_list.append(imp_clean)

    imports_block = "\n".join(sorted(imports_list))

    # 2. Substitute and order templates
    body_blocks: List[str] = []
    for snippet_id in ORDERED_SNIPPET_IDS:
        template = snippets_by_id[snippet_id]["template"]
        substituted = substitute_template(template, DUMMY_VALUES)
        body_blocks.append(substituted)

    # 3. Combine body and finalize router attachment
    full_code = imports_block + "\n\n\n" + "\n\n\n".join(body_blocks) + "\n\n\napp.include_router(router)\n"
    return full_code


def main() -> int:
    try:
        snippets_dir = find_snippets_dir()
        print(f"[*] Found snippets directory at: {snippets_dir}")

        snippets_by_id: Dict[str, Dict[str, Any]] = {}

        # 1. Validate existence and schema
        for category, snippet_ids in REQUIRED_SNIPPETS.items():
            cat_dir = snippets_dir / category
            if not cat_dir.is_dir():
                raise FileNotFoundError(f"Missing category directory: {cat_dir}")

            for s_id in snippet_ids:
                s_file = cat_dir / f"{s_id}.json"
                if not s_file.is_file():
                    raise FileNotFoundError(f"Missing snippet file: {s_file}")

                with open(s_file, "r", encoding="utf-8") as f:
                    data = json.load(f)

                validate_snippet_schema(data, s_id, s_file)
                snippets_by_id[s_id] = data
                print(f"  [+] Validated schema: {category}/{s_id}.json")

        # 2. Assemble mock main.py
        mock_code = assemble_mock_main(snippets_by_id)

        # 3. Tier 1: AST syntax validation
        ast.parse(mock_code)
        print("  [+] AST parsing check passed (syntactically valid Python).")

        # 4. Tier 2: py_compile execution
        with tempfile.NamedTemporaryFile("w", suffix=".py", delete=False) as tf:
            tf.write(mock_code)
            tf_path = tf.name

        try:
            res = subprocess.run(
                [sys.executable, "-m", "py_compile", tf_path],
                capture_output=True,
                text=True,
                check=True,
            )
            print("  [+] Bytecode py_compile check passed with exit code 0.")
        finally:
            if os.path.exists(tf_path):
                os.remove(tf_path)

        print("\n[SUCCESS] All 8 snippets validated, assembled, and successfully compiled!")
        return 0

    except Exception as e:
        print(f"\n[ERROR] Verification failed: {e}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
