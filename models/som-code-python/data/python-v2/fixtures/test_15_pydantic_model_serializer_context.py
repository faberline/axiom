import inspect
import pytest
from candidate import CustomerRecord


@pytest.fixture
def sample_customer():
    return CustomerRecord(
        customer_id="cust_101",
        name="Jane Doe",
        credit_card="4111222233331234",
        ssn="123-45-6789",
        notes="High net worth private client",
    )


def test_admin_role_full_data_serialization(sample_customer):
    dumped = sample_customer.model_dump(context={"role": "admin"})
    assert dumped["customer_id"] == "cust_101"
    assert dumped["name"] == "Jane Doe"
    assert dumped["credit_card"] == "4111222233331234"
    assert dumped["ssn"] == "123-45-6789"
    assert dumped["notes"] == "High net worth private client"


def test_auditor_role_masked_cc_and_ssn_with_notes(sample_customer):
    dumped = sample_customer.model_dump(context={"role": "auditor"})
    assert dumped["customer_id"] == "cust_101"
    assert dumped["credit_card"] == "****-****-****-1234"
    assert dumped["ssn"] == "***-**-****"
    assert dumped["notes"] == "High net worth private client"


def test_public_role_strips_ssn_and_notes_and_masks_cc(sample_customer):
    # Catches miss_3 (wrong_branch): 'if role == "admin" or "auditor":' leaks raw data
    dumped = sample_customer.model_dump(context={"role": "public"})
    assert dumped["customer_id"] == "cust_101"
    assert dumped["credit_card"] == "****-****-****-1234"
    assert "ssn" not in dumped, "SSN must be stripped for public role"
    assert "notes" not in dumped, "Notes must be stripped for public role"


def test_dump_without_context_defaults_to_public(sample_customer):
    # Catches miss_1 (missing_validation): crashes on info.context.get() when info.context is None
    dumped = sample_customer.model_dump()
    assert dumped["customer_id"] == "cust_101"
    assert dumped["credit_card"] == "****-****-****-1234"
    assert "ssn" not in dumped
    assert "notes" not in dumped


def test_serializer_signature_and_context_handling(sample_customer):
    # Catches miss_2 (wrong_api_call): defines signature as (self, handler) omitting info
    sig = inspect.signature(CustomerRecord.serialize_model)
    assert "info" in sig.parameters, "Serializer signature must accept SerializationInfo 'info' parameter"

    dumped = sample_customer.model_dump(context={"role": "public"})
    assert dumped["credit_card"] == "****-****-****-1234"


def test_masking_boundary_exposes_last_four_digits(sample_customer):
    # Catches miss_4 (wrong_boundary): exposes first 4 digits instead of last 4
    dumped = sample_customer.model_dump(context={"role": "auditor"})
    assert dumped["credit_card"] == "****-****-****-1234"
    assert not dumped["credit_card"].endswith("4111"), "Credit card mask must not expose the first 4 digits"


def test_empty_context_dict_defaults_to_public_role(sample_customer):
    # Catches miss_5 (wrong_default): defaults role to 'admin' instead of 'public'
    dumped = sample_customer.model_dump(context={})
    assert "ssn" not in dumped, "Empty context must default to public role and omit SSN"
    assert dumped["credit_card"] == "****-****-****-1234"
