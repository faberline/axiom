//! Comprehensive validation tests

use schema::constraints::*;
use schema::types::*;
use schema::validate;

// ============================================================================
// String Validation Tests
// ============================================================================

#[test]
fn test_string_basic() {
    let type_desc = TypeDescriptor::String(Default::default());
    let value = Value::String("hello".to_string());
    assert!(validate(&value, &type_desc).is_ok());

    // Wrong type
    let wrong = Value::Int(42);
    assert!(validate(&wrong, &type_desc).is_err());
}

#[test]
fn test_string_min_length() {
    let constraints = StringConstraints {
        min_length: Some(5),
        ..Default::default()
    };
    let type_desc = TypeDescriptor::String(constraints);

    // Valid
    let valid = Value::String("hello".to_string());
    assert!(validate(&valid, &type_desc).is_ok());

    // Too short
    let too_short = Value::String("hi".to_string());
    assert!(validate(&too_short, &type_desc).is_err());
}

#[test]
fn test_string_max_length() {
    let constraints = StringConstraints {
        max_length: Some(5),
        ..Default::default()
    };
    let type_desc = TypeDescriptor::String(constraints);

    // Valid
    let valid = Value::String("hello".to_string());
    assert!(validate(&valid, &type_desc).is_ok());

    // Too long
    let too_long = Value::String("hello world".to_string());
    assert!(validate(&too_long, &type_desc).is_err());
}

#[test]
fn test_string_pattern() {
    let constraints = StringConstraints {
        pattern: Some(r"^\d{3}-\d{4}$".to_string()),
        ..Default::default()
    };
    let type_desc = TypeDescriptor::String(constraints);

    // Valid
    let valid = Value::String("123-4567".to_string());
    assert!(validate(&valid, &type_desc).is_ok());

    // Invalid
    let invalid = Value::String("abc-defg".to_string());
    assert!(validate(&invalid, &type_desc).is_err());
}

// ============================================================================
// Numeric Validation Tests
// ============================================================================

#[test]
fn test_int64_basic() {
    let type_desc = TypeDescriptor::Int64(Default::default());
    let value = Value::Int(42);
    assert!(validate(&value, &type_desc).is_ok());

    // Wrong type
    let wrong = Value::String("42".to_string());
    assert!(validate(&wrong, &type_desc).is_err());
}

#[test]
fn test_int64_min_max() {
    let constraints = NumericConstraints {
        minimum: Some(0),
        maximum: Some(100),
        ..Default::default()
    };
    let type_desc = TypeDescriptor::Int64(constraints);

    // Valid
    assert!(validate(&Value::Int(50), &type_desc).is_ok());
    assert!(validate(&Value::Int(0), &type_desc).is_ok());
    assert!(validate(&Value::Int(100), &type_desc).is_ok());

    // Invalid
    assert!(validate(&Value::Int(-1), &type_desc).is_err());
    assert!(validate(&Value::Int(101), &type_desc).is_err());
}

#[test]
fn test_int64_exclusive_min_max() {
    let constraints = NumericConstraints {
        exclusive_minimum: Some(0),
        exclusive_maximum: Some(100),
        ..Default::default()
    };
    let type_desc = TypeDescriptor::Int64(constraints);

    // Valid
    assert!(validate(&Value::Int(50), &type_desc).is_ok());
    assert!(validate(&Value::Int(1), &type_desc).is_ok());
    assert!(validate(&Value::Int(99), &type_desc).is_ok());

    // Invalid (boundaries)
    assert!(validate(&Value::Int(0), &type_desc).is_err());
    assert!(validate(&Value::Int(100), &type_desc).is_err());
}

#[test]
fn test_int64_multiple_of() {
    let constraints = NumericConstraints {
        multiple_of: Some(5),
        ..Default::default()
    };
    let type_desc = TypeDescriptor::Int64(constraints);

    // Valid
    assert!(validate(&Value::Int(10), &type_desc).is_ok());
    assert!(validate(&Value::Int(15), &type_desc).is_ok());

    // Invalid
    assert!(validate(&Value::Int(7), &type_desc).is_err());
}

#[test]
fn test_float64_basic() {
    let type_desc = TypeDescriptor::Float64(Default::default());
    assert!(validate(&Value::Float(3.14), &type_desc).is_ok());
    assert!(validate(&Value::Int(42), &type_desc).is_ok()); // Int coerced to float
}

#[test]
fn test_float64_min_max() {
    let constraints = NumericConstraints {
        minimum: Some(0.0),
        maximum: Some(100.0),
        ..Default::default()
    };
    let type_desc = TypeDescriptor::Float64(constraints);

    assert!(validate(&Value::Float(50.5), &type_desc).is_ok());
    assert!(validate(&Value::Float(-0.1), &type_desc).is_err());
    assert!(validate(&Value::Float(100.1), &type_desc).is_err());
}

// ============================================================================
// Boolean and Null Tests
// ============================================================================

#[test]
fn test_bool() {
    let type_desc = TypeDescriptor::Bool;
    assert!(validate(&Value::Bool(true), &type_desc).is_ok());
    assert!(validate(&Value::Bool(false), &type_desc).is_ok());
    assert!(validate(&Value::Int(1), &type_desc).is_err());
}

#[test]
fn test_null() {
    let type_desc = TypeDescriptor::Null;
    assert!(validate(&Value::Null, &type_desc).is_ok());
    assert!(validate(&Value::Int(0), &type_desc).is_err());
}

// ============================================================================
// Collection Types Tests
// ============================================================================

#[test]
fn test_list_basic() {
    let type_desc = TypeDescriptor::List {
        items: Box::new(TypeDescriptor::Int64(Default::default())),
        constraints: Default::default(),
    };

    let valid = Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)]);
    assert!(validate(&valid, &type_desc).is_ok());

    let invalid = Value::List(vec![Value::Int(1), Value::String("two".to_string())]);
    assert!(validate(&invalid, &type_desc).is_err());
}

#[test]
fn test_list_min_max_items() {
    let constraints = ListConstraints {
        min_items: Some(2),
        max_items: Some(4),
        unique_items: false,
    };
    let type_desc = TypeDescriptor::List {
        items: Box::new(TypeDescriptor::Int64(Default::default())),
        constraints,
    };

    // Valid
    assert!(validate(&Value::List(vec![Value::Int(1), Value::Int(2)]), &type_desc).is_ok());

    // Too few items
    assert!(validate(&Value::List(vec![Value::Int(1)]), &type_desc).is_err());

    // Too many items
    assert!(validate(
        &Value::List(vec![
            Value::Int(1),
            Value::Int(2),
            Value::Int(3),
            Value::Int(4),
            Value::Int(5)
        ]),
        &type_desc
    )
    .is_err());
}

#[test]
fn test_list_unique_items() {
    let constraints = ListConstraints {
        unique_items: true,
        ..Default::default()
    };
    let type_desc = TypeDescriptor::List {
        items: Box::new(TypeDescriptor::Int64(Default::default())),
        constraints,
    };

    // Valid (all unique)
    assert!(validate(
        &Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)]),
        &type_desc
    )
    .is_ok());

    // Invalid (duplicates)
    assert!(validate(
        &Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(1)]),
        &type_desc
    )
    .is_err());
}

#[test]
fn test_tuple() {
    let type_desc = TypeDescriptor::Tuple {
        items: vec![
            TypeDescriptor::Int64(Default::default()),
            TypeDescriptor::String(Default::default()),
            TypeDescriptor::Bool,
        ],
    };

    // Valid
    let valid = Value::List(vec![
        Value::Int(42),
        Value::String("hello".to_string()),
        Value::Bool(true),
    ]);
    assert!(validate(&valid, &type_desc).is_ok());

    // Wrong length
    let wrong_length = Value::List(vec![Value::Int(42), Value::String("hello".to_string())]);
    assert!(validate(&wrong_length, &type_desc).is_err());

    // Wrong types
    let wrong_types = Value::List(vec![
        Value::String("42".to_string()),
        Value::Int(123),
        Value::Bool(true),
    ]);
    assert!(validate(&wrong_types, &type_desc).is_err());
}

#[test]
fn test_set() {
    let type_desc = TypeDescriptor::Set {
        items: Box::new(TypeDescriptor::Int64(Default::default())),
    };

    // Valid (all unique)
    assert!(validate(
        &Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)]),
        &type_desc
    )
    .is_ok());

    // Invalid (duplicates)
    assert!(validate(
        &Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(1)]),
        &type_desc
    )
    .is_err());
}

#[test]
fn test_object() {
    let type_desc = TypeDescriptor::Object {
        fields: vec![
            FieldDescriptor::new("name", TypeDescriptor::String(Default::default())),
            FieldDescriptor::new("age", TypeDescriptor::Int64(Default::default())),
        ],
        additional: None,
    };

    // Valid
    let valid = Value::Object(vec![
        ("name".to_string(), Value::String("Alice".to_string())),
        ("age".to_string(), Value::Int(30)),
    ]);
    assert!(validate(&valid, &type_desc).is_ok());

    // Missing required field
    let missing = Value::Object(vec![("name".to_string(), Value::String("Bob".to_string()))]);
    assert!(validate(&missing, &type_desc).is_err());
}

// ============================================================================
// Special Types Tests
// ============================================================================

#[test]
fn test_optional() {
    let type_desc = TypeDescriptor::Optional(Box::new(TypeDescriptor::Int64(Default::default())));

    // Null is allowed
    assert!(validate(&Value::Null, &type_desc).is_ok());

    // Valid value
    assert!(validate(&Value::Int(42), &type_desc).is_ok());

    // Invalid value
    assert!(validate(&Value::String("42".to_string()), &type_desc).is_err());
}

#[test]
fn test_union() {
    let type_desc = TypeDescriptor::Union {
        variants: vec![
            TypeDescriptor::Int64(Default::default()),
            TypeDescriptor::String(Default::default()),
        ],
        nullable: false,
    };

    // Valid (matches first variant)
    assert!(validate(&Value::Int(42), &type_desc).is_ok());

    // Valid (matches second variant)
    assert!(validate(&Value::String("hello".to_string()), &type_desc).is_ok());

    // Invalid (matches no variant)
    assert!(validate(&Value::Bool(true), &type_desc).is_err());

    // Null not allowed
    assert!(validate(&Value::Null, &type_desc).is_err());
}

#[test]
fn test_union_nullable() {
    let type_desc = TypeDescriptor::Union {
        variants: vec![TypeDescriptor::Int64(Default::default())],
        nullable: true,
    };

    // Null is allowed
    assert!(validate(&Value::Null, &type_desc).is_ok());

    // Value is also allowed
    assert!(validate(&Value::Int(42), &type_desc).is_ok());
}

#[test]
fn test_enum() {
    let type_desc = TypeDescriptor::Enum {
        values: vec![
            Value::String("red".to_string()),
            Value::String("green".to_string()),
            Value::String("blue".to_string()),
        ],
    };

    // Valid
    assert!(validate(&Value::String("red".to_string()), &type_desc).is_ok());

    // Invalid
    assert!(validate(&Value::String("yellow".to_string()), &type_desc).is_err());
}

#[test]
fn test_literal() {
    let type_desc = TypeDescriptor::Literal {
        values: vec![Value::Int(42), Value::String("answer".to_string())],
    };

    // Valid
    assert!(validate(&Value::Int(42), &type_desc).is_ok());
    assert!(validate(&Value::String("answer".to_string()), &type_desc).is_ok());

    // Invalid
    assert!(validate(&Value::Int(43), &type_desc).is_err());
}

// ============================================================================
// Format Types Tests
// ============================================================================

#[test]
fn test_email() {
    let type_desc = TypeDescriptor::Email;

    // Valid
    assert!(validate(&Value::String("user@example.com".to_string()), &type_desc).is_ok());

    // Invalid
    assert!(validate(&Value::String("not-an-email".to_string()), &type_desc).is_err());

    // Wrong type
    assert!(validate(&Value::Int(123), &type_desc).is_err());
}

#[test]
fn test_url() {
    let type_desc = TypeDescriptor::Url;

    // Valid
    assert!(validate(
        &Value::String("https://example.com".to_string()),
        &type_desc
    )
    .is_ok());
    assert!(validate(
        &Value::String("http://localhost:8080/path".to_string()),
        &type_desc
    )
    .is_ok());

    // Invalid
    assert!(validate(&Value::String("not-a-url".to_string()), &type_desc).is_err());
}

#[test]
fn test_uuid() {
    let type_desc = TypeDescriptor::Uuid;

    // Valid (UUID v4)
    assert!(validate(
        &Value::String("550e8400-e29b-41d4-a716-446655440000".to_string()),
        &type_desc
    )
    .is_ok());

    // Invalid
    assert!(validate(&Value::String("not-a-uuid".to_string()), &type_desc).is_err());
}

#[test]
fn test_datetime() {
    let type_desc = TypeDescriptor::DateTime;

    // Valid
    assert!(validate(
        &Value::String("2024-01-19T12:00:00Z".to_string()),
        &type_desc
    )
    .is_ok());
    assert!(validate(
        &Value::String("2024-01-19T12:00:00+08:00".to_string()),
        &type_desc
    )
    .is_ok());

    // Invalid
    assert!(validate(
        &Value::String("2024-01-19 12:00:00".to_string()),
        &type_desc
    )
    .is_err());
}

#[test]
fn test_date() {
    let type_desc = TypeDescriptor::Date;

    // Valid
    assert!(validate(&Value::String("2024-01-19".to_string()), &type_desc).is_ok());

    // Invalid
    assert!(validate(&Value::String("01/19/2024".to_string()), &type_desc).is_err());
}

#[test]
fn test_time() {
    let type_desc = TypeDescriptor::Time;

    // Valid
    assert!(validate(&Value::String("12:00:00".to_string()), &type_desc).is_ok());
    assert!(validate(&Value::String("23:59:59.999".to_string()), &type_desc).is_ok());

    // Invalid
    assert!(validate(&Value::String("25:00:00".to_string()), &type_desc).is_err());
}

// ============================================================================
// Any Type Test
// ============================================================================

#[test]
fn test_any() {
    let type_desc = TypeDescriptor::Any;

    // Everything is valid
    assert!(validate(&Value::Null, &type_desc).is_ok());
    assert!(validate(&Value::Int(42), &type_desc).is_ok());
    assert!(validate(&Value::String("hello".to_string()), &type_desc).is_ok());
    assert!(validate(&Value::Bool(true), &type_desc).is_ok());
}

// ============================================================================
// Nested Structures Tests
// ============================================================================

#[test]
fn test_nested_object() {
    let type_desc = TypeDescriptor::Object {
        fields: vec![FieldDescriptor::new(
            "user",
            TypeDescriptor::Object {
                fields: vec![
                    FieldDescriptor::new("name", TypeDescriptor::String(Default::default())),
                    FieldDescriptor::new("email", TypeDescriptor::Email),
                ],
                additional: None,
            },
        )],
        additional: None,
    };

    // Valid
    let valid = Value::Object(vec![(
        "user".to_string(),
        Value::Object(vec![
            ("name".to_string(), Value::String("Alice".to_string())),
            (
                "email".to_string(),
                Value::String("alice@example.com".to_string()),
            ),
        ]),
    )]);
    assert!(validate(&valid, &type_desc).is_ok());

    // Invalid email in nested object
    let invalid = Value::Object(vec![(
        "user".to_string(),
        Value::Object(vec![
            ("name".to_string(), Value::String("Bob".to_string())),
            (
                "email".to_string(),
                Value::String("not-an-email".to_string()),
            ),
        ]),
    )]);
    assert!(validate(&invalid, &type_desc).is_err());
}

#[test]
fn test_nested_list() {
    let type_desc = TypeDescriptor::List {
        items: Box::new(TypeDescriptor::List {
            items: Box::new(TypeDescriptor::Int64(Default::default())),
            constraints: Default::default(),
        }),
        constraints: Default::default(),
    };

    // Valid
    let valid = Value::List(vec![
        Value::List(vec![Value::Int(1), Value::Int(2)]),
        Value::List(vec![Value::Int(3), Value::Int(4)]),
    ]);
    assert!(validate(&valid, &type_desc).is_ok());

    // Invalid (wrong type in nested list)
    let invalid = Value::List(vec![
        Value::List(vec![Value::Int(1), Value::String("two".to_string())]),
        Value::List(vec![Value::Int(3)]),
    ]);
    assert!(validate(&invalid, &type_desc).is_err());
}
