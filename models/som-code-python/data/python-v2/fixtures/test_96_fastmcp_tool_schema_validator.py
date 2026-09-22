import pytest
from typing import Optional
import candidate
from candidate import FastMCPToolSchemaValidator


def sample_tool(
    query: str,
    limit: int = 10,
    tags: list[str] = None,
    is_active: bool = True,
) -> str:
    """Search for relevant records in database."""
    return query


def test_strict_schema_has_additional_properties_false():
    validator = FastMCPToolSchemaValidator(strict_mode=True)
    schema = validator.generate_schema_for_function(sample_tool)
    params = schema["function"]["parameters"]
    assert params.get("additionalProperties") is False, "Strict mode must enforce additionalProperties=False"

    is_valid, errors = validator.validate_schema(schema)
    assert is_valid is True, f"Generated schema failed validation: {errors}"


def test_array_parameter_includes_items_schema():
    validator = FastMCPToolSchemaValidator(strict_mode=True)
    schema = validator.generate_schema_for_function(sample_tool)
    tags_prop = schema["function"]["parameters"]["properties"]["tags"]
    assert tags_prop.get("type") == "array"
    assert "items" in tags_prop, "Array properties must define an 'items' schema"
    assert tags_prop["items"].get("type") == "string"


def test_default_strict_mode_is_true():
    validator = FastMCPToolSchemaValidator()
    assert validator.strict_mode is True, "Default strict_mode must be True"


def test_strict_mode_requires_all_parameters():
    validator = FastMCPToolSchemaValidator(strict_mode=True)
    schema = validator.generate_schema_for_function(sample_tool)
    required = schema["function"]["parameters"]["required"]
    # In strict mode, even parameters with defaults (limit, tags, is_active) must be in required
    assert set(required) == {"query", "limit", "tags", "is_active"}, (
        f"Strict schema required list must contain all parameters, got {required}"
    )


def test_top_level_parameters_type_is_object():
    validator = FastMCPToolSchemaValidator(strict_mode=True)
    schema = validator.generate_schema_for_function(sample_tool)
    params = schema["function"]["parameters"]
    assert params.get("type") == "object", f"Top-level parameters type must be 'object', got {params.get('type')}"


def test_validate_schema_reports_errors():
    validator = FastMCPToolSchemaValidator(strict_mode=True)
    bad_schema = {
        "function": {
            "name": "bad",
            "parameters": {
                "type": "object",
                "properties": {
                    "foo": {},  # missing type
                    "bar": {"type": "array"},  # missing items
                },
                "required": ["foo"],  # missing bar in strict mode
                "additionalProperties": True,  # should be False
            },
        }
    }
    is_valid, errors = validator.validate_schema(bad_schema)
    assert is_valid is False
    assert len(errors) >= 3
