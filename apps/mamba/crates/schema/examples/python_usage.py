#!/usr/bin/env python3
"""
Python Usage Examples for cclab.shield

This example demonstrates how to use the cclab.shield module
from Python code. The validation is performed in Rust for maximum performance
while providing a Pydantic-like API.
"""

from cclab.shield import validate

def example_email_validation():
    """Example: Email format validation"""
    print("=== Email Validation ===")

    type_desc = {"type": "email"}

    # Valid email
    try:
        validate("user@example.com", type_desc)
        print("✅ 'user@example.com' is valid")
    except ValueError as e:
        print(f"❌ Error: {e}")

    # Invalid email
    try:
        validate("not-an-email", type_desc)
        print("✅ 'not-an-email' is valid")
    except ValueError as e:
        print(f"❌ 'not-an-email' failed: {e}")

    print()


def example_string_constraints():
    """Example: String with length constraints"""
    print("=== String Constraints ===")

    type_desc = {
        "type": "string",
        "constraints": {
            "min_length": 3,
            "max_length": 20,
            "pattern": "^[a-z]+$"  # lowercase letters only
        }
    }

    # Valid string
    try:
        validate("hello", type_desc)
        print("✅ 'hello' is valid")
    except ValueError as e:
        print(f"❌ Error: {e}")

    # Too short
    try:
        validate("hi", type_desc)
        print("✅ 'hi' is valid")
    except ValueError as e:
        print(f"❌ 'hi' failed: {e}")

    # Pattern mismatch
    try:
        validate("Hello", type_desc)
        print("✅ 'Hello' is valid")
    except ValueError as e:
        print(f"❌ 'Hello' failed: {e}")

    print()


def example_numeric_constraints():
    """Example: Numeric constraints"""
    print("=== Numeric Constraints ===")

    type_desc = {
        "type": "int64",
        "constraints": {
            "minimum": 0,
            "maximum": 100,
            "multiple_of": 5
        }
    }

    # Valid numbers
    for num in [0, 50, 100]:
        try:
            validate(num, type_desc)
            print(f"✅ {num} is valid")
        except ValueError as e:
            print(f"❌ {num} failed: {e}")

    # Invalid numbers
    for num in [-5, 101, 33]:
        try:
            validate(num, type_desc)
            print(f"✅ {num} is valid")
        except ValueError as e:
            print(f"❌ {num} failed: {e}")

    print()


def example_list_validation():
    """Example: List validation with constraints"""
    print("=== List Validation ===")

    type_desc = {
        "type": "list",
        "items": {"type": "string"},
        "constraints": {
            "min_items": 1,
            "max_items": 5,
            "unique_items": True
        }
    }

    # Valid lists
    for lst in [["a"], ["a", "b", "c"], ["x", "y", "z", "w", "v"]]:
        try:
            validate(lst, type_desc)
            print(f"✅ {lst} is valid")
        except ValueError as e:
            print(f"❌ {lst} failed: {e}")

    # Invalid lists
    for lst in [[], ["a", "b", "c", "d", "e", "f"], ["a", "a", "b"]]:
        try:
            validate(lst, type_desc)
            print(f"✅ {lst} is valid")
        except ValueError as e:
            print(f"❌ {lst} failed: {e}")

    print()


def example_object_validation():
    """Example: Object validation with required and optional fields"""
    print("=== Object Validation ===")

    type_desc = {
        "type": "object",
        "fields": [
            {
                "name": "email",
                "type": {"type": "email"},
                "required": True
            },
            {
                "name": "age",
                "type": {
                    "type": "int64",
                    "constraints": {"minimum": 0, "maximum": 150}
                },
                "required": False,
                "default": 0
            },
            {
                "name": "name",
                "type": {
                    "type": "string",
                    "constraints": {"min_length": 1, "max_length": 100}
                },
                "required": True
            }
        ]
    }

    # Valid objects
    valid_objs = [
        {"email": "user@example.com", "name": "John", "age": 30},
        {"email": "alice@test.com", "name": "Alice"},  # age is optional
    ]

    for obj in valid_objs:
        try:
            validate(obj, type_desc)
            print(f"✅ {obj} is valid")
        except ValueError as e:
            print(f"❌ {obj} failed: {e}")

    # Invalid objects
    invalid_objs = [
        {"name": "John", "age": 30},  # missing required email
        {"email": "not-an-email", "name": "John"},  # invalid email format
        {"email": "user@example.com", "name": "John", "age": 200},  # age out of range
    ]

    for obj in invalid_objs:
        try:
            validate(obj, type_desc)
            print(f"✅ {obj} is valid")
        except ValueError as e:
            print(f"❌ {obj} failed: {e}")

    print()


def example_union_validation():
    """Example: Union type (value can be one of multiple types)"""
    print("=== Union Validation ===")

    type_desc = {
        "type": "union",
        "variants": [
            {"type": "string"},
            {"type": "int64"}
        ],
        "nullable": False
    }

    # Valid values
    for val in ["hello", 42]:
        try:
            validate(val, type_desc)
            print(f"✅ {val} ({type(val).__name__}) is valid")
        except ValueError as e:
            print(f"❌ {val} failed: {e}")

    # Invalid values
    for val in [3.14, None, []]:
        try:
            validate(val, type_desc)
            print(f"✅ {val} ({type(val).__name__}) is valid")
        except ValueError as e:
            print(f"❌ {val} ({type(val).__name__}) failed: {e}")

    print()


def example_optional_validation():
    """Example: Optional type (nullable)"""
    print("=== Optional Validation ===")

    type_desc = {
        "type": "optional",
        "inner": {"type": "string"}
    }

    # Valid values
    for val in ["hello", None]:
        try:
            validate(val, type_desc)
            print(f"✅ {val} is valid")
        except ValueError as e:
            print(f"❌ {val} failed: {e}")

    # Invalid value
    try:
        validate(42, type_desc)
        print(f"✅ 42 is valid")
    except ValueError as e:
        print(f"❌ 42 failed: {e}")

    print()


def example_enum_validation():
    """Example: Enum type (one of specific values)"""
    print("=== Enum Validation ===")

    type_desc = {
        "type": "enum",
        "values": ["red", "green", "blue"]
    }

    # Valid values
    for val in ["red", "green", "blue"]:
        try:
            validate(val, type_desc)
            print(f"✅ '{val}' is valid")
        except ValueError as e:
            print(f"❌ '{val}' failed: {e}")

    # Invalid value
    try:
        validate("yellow", type_desc)
        print(f"✅ 'yellow' is valid")
    except ValueError as e:
        print(f"❌ 'yellow' failed: {e}")

    print()


def example_format_types():
    """Example: Format types (url, uuid, datetime, etc.)"""
    print("=== Format Types ===")

    formats = [
        ("url", ["https://example.com", "invalid-url"]),
        ("uuid", ["550e8400-e29b-41d4-a716-446655440000", "not-a-uuid"]),
        ("datetime", ["2024-01-19T10:30:00Z", "invalid-datetime"]),
        ("date", ["2024-01-19", "2024-13-45"]),
        ("time", ["10:30:00", "25:99:99"]),
    ]

    for format_type, values in formats:
        print(f"\n{format_type.upper()}:")
        type_desc = {"type": format_type}

        for val in values:
            try:
                validate(val, type_desc)
                print(f"  ✅ '{val}' is valid")
            except ValueError as e:
                print(f"  ❌ '{val}' failed: {e}")

    print()


def main():
    """Run all examples"""
    print("=" * 60)
    print("cclab.shield - Python Usage Examples")
    print("=" * 60)
    print()

    example_email_validation()
    example_string_constraints()
    example_numeric_constraints()
    example_list_validation()
    example_object_validation()
    example_union_validation()
    example_optional_validation()
    example_enum_validation()
    example_format_types()

    print("=" * 60)
    print("All examples completed!")
    print("=" * 60)


if __name__ == "__main__":
    main()
