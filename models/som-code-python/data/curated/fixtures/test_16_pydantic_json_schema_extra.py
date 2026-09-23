import pytest
from pydantic import ValidationError
from candidate import PaymentTransaction


def test_gold_json_schema_customization():
    schema = PaymentTransaction.model_json_schema()
    assert "examples" in schema, "Schema must contain 'examples' key"
    assert isinstance(schema["examples"], list) and len(schema["examples"]) > 0

    assert "x-metadata" in schema, "Schema must contain 'x-metadata' key"
    assert schema["x-metadata"]["owner"] == "billing-core"
    assert schema["x-metadata"]["classification"] == "restricted"

    assert "properties" in schema, "Schema must contain 'properties'"
    assert "account_number" in schema["properties"]
    assert schema["properties"]["account_number"].get("x-pii") is True, "Sensitive account_number must have x-pii: True"


def test_json_schema_extra_v2_configuration():
    # Catches miss_1 (wrong_api_call): uses legacy V1 class Config: schema_extra which V2 ignores
    schema = PaymentTransaction.model_json_schema()
    assert "examples" in schema, "json_schema_extra examples missing; verify Pydantic V2 ConfigDict is used"
    assert "x-metadata" in schema, "json_schema_extra x-metadata missing; verify Pydantic V2 ConfigDict is used"
    assert "x-metadata" in schema and schema["x-metadata"].get("owner") == "billing-core"


def test_augment_schema_callable_on_fresh_schema():
    # Catches miss_2 (missing_validation): assumes schema['examples'] already exists causing KeyError
    schema = PaymentTransaction.model_json_schema()
    assert "examples" in schema
    assert isinstance(schema["examples"], list)


def test_schema_retains_properties_and_structure():
    # Catches miss_3 (wrong_branch): replaces schema with metadata-only dict, dropping properties
    schema = PaymentTransaction.model_json_schema()
    assert "properties" in schema, "Augmented schema must retain core model properties"
    assert "amount" in schema["properties"]
    assert "transaction_id" in schema["properties"]
    assert "account_number" in schema["properties"]


def test_examples_conform_to_model_boundary_constraints():
    # Catches miss_4 (wrong_boundary): injects example with amount: 0.0 violating gt=0.0
    schema = PaymentTransaction.model_json_schema()
    examples = schema.get("examples", [])
    assert len(examples) > 0, "At least one example must be present in schema"
    for ex in examples:
        instance = PaymentTransaction.model_validate(ex)
        assert instance.amount > 0.0, f"Example amount must be strictly positive, got {instance.amount}"


def test_classification_metadata_is_restricted():
    # Catches miss_5 (wrong_default): sets classification to 'public' instead of 'restricted'
    schema = PaymentTransaction.model_json_schema()
    x_meta = schema.get("x-metadata", {})
    assert x_meta.get("classification") == "restricted", f"Expected classification 'restricted', got {x_meta.get('classification')!r}"
