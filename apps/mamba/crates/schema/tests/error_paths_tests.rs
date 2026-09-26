//! Error Paths & Messages Tests
//!
//! Tests for validation error paths, messages, and formatting.
//! Ensures Pydantic-compatible error reporting.

use schema::constraints::*;
use schema::errors::*;
use schema::types::*;
use schema::validate;

// ============================================================================
// Simple Field Error Paths
// ============================================================================

#[test]
fn test_error_path_simple_field() {
    let type_desc = TypeDescriptor::Object {
        fields: vec![
            FieldDescriptor::new("name", TypeDescriptor::String(Default::default())),
            FieldDescriptor::new("age", TypeDescriptor::Int64(Default::default())),
        ],
        additional: None,
    };

    // Wrong type for 'age' field
    let value = Value::Object(vec![
        ("name".to_string(), Value::String("Alice".to_string())),
        ("age".to_string(), Value::String("not a number".to_string())),
    ]);

    let result = validate(&value, &type_desc);
    assert!(result.is_err());

    let errors = result.unwrap_err();
    assert_eq!(errors.len(), 1);

    let error = &errors.errors[0];
    assert!(error.field.contains("age") || error.location.contains("age"));
}

#[test]
fn test_error_path_nested_object() {
    let type_desc = TypeDescriptor::Object {
        fields: vec![FieldDescriptor::new(
            "user",
            TypeDescriptor::Object {
                fields: vec![FieldDescriptor::new(
                    "address",
                    TypeDescriptor::Object {
                        fields: vec![FieldDescriptor::new(
                            "city",
                            TypeDescriptor::String(Default::default()),
                        )],
                        additional: None,
                    },
                )],
                additional: None,
            },
        )],
        additional: None,
    };

    // Wrong type for nested 'city' field
    let value = Value::Object(vec![(
        "user".to_string(),
        Value::Object(vec![(
            "address".to_string(),
            Value::Object(vec![("city".to_string(), Value::Int(12345))]),
        )]),
    )]);

    let result = validate(&value, &type_desc);
    assert!(result.is_err());

    let errors = result.unwrap_err();
    assert!(!errors.is_empty());
    // The error should reference the nested path
    let error = &errors.errors[0];
    // Path should contain city or nested reference
    assert!(
        error.field.contains("city")
            || error.location.contains("city")
            || error.location.contains("address")
            || error.location.contains("user")
    );
}

#[test]
fn test_error_path_array_index() {
    let type_desc = TypeDescriptor::List {
        items: Box::new(TypeDescriptor::Int64(Default::default())),
        constraints: Default::default(),
    };

    // Mixed types in array - second element is wrong
    let value = Value::List(vec![
        Value::Int(1),
        Value::String("not a number".to_string()),
        Value::Int(3),
    ]);

    let result = validate(&value, &type_desc);
    assert!(result.is_err());

    let errors = result.unwrap_err();
    assert!(!errors.is_empty());
    // Error should reference array index
    let error = &errors.errors[0];
    assert!(
        error.field.contains("1")
            || error.field.contains("[1]")
            || error.location.contains("1")
            || error.location.contains("[1]")
    );
}

#[test]
fn test_error_path_deep_nesting() {
    // Create a deeply nested structure: root.level1.level2.level3.value
    let type_desc = TypeDescriptor::Object {
        fields: vec![FieldDescriptor::new(
            "level1",
            TypeDescriptor::Object {
                fields: vec![FieldDescriptor::new(
                    "level2",
                    TypeDescriptor::Object {
                        fields: vec![FieldDescriptor::new(
                            "level3",
                            TypeDescriptor::Object {
                                fields: vec![FieldDescriptor::new(
                                    "value",
                                    TypeDescriptor::Int64(Default::default()),
                                )],
                                additional: None,
                            },
                        )],
                        additional: None,
                    },
                )],
                additional: None,
            },
        )],
        additional: None,
    };

    // Wrong type at deepest level
    let value = Value::Object(vec![(
        "level1".to_string(),
        Value::Object(vec![(
            "level2".to_string(),
            Value::Object(vec![(
                "level3".to_string(),
                Value::Object(vec![("value".to_string(), Value::Bool(true))]),
            )]),
        )]),
    )]);

    let result = validate(&value, &type_desc);
    assert!(result.is_err());

    let errors = result.unwrap_err();
    assert!(!errors.is_empty());
}

// ============================================================================
// Multiple Errors Collection
// ============================================================================

#[test]
fn test_multiple_errors_collection() {
    let type_desc = TypeDescriptor::Object {
        fields: vec![
            FieldDescriptor::new("name", TypeDescriptor::String(Default::default())),
            FieldDescriptor::new("email", TypeDescriptor::Email),
            FieldDescriptor::new("age", TypeDescriptor::Int64(Default::default())),
        ],
        additional: None,
    };

    // Multiple errors: wrong type for name, invalid email, missing age
    let value = Value::Object(vec![
        ("name".to_string(), Value::Int(123)),
        (
            "email".to_string(),
            Value::String("not-an-email".to_string()),
        ),
        // age is missing
    ]);

    let result = validate(&value, &type_desc);
    assert!(result.is_err());

    let errors = result.unwrap_err();
    // Should collect multiple errors
    assert!(
        errors.len() >= 2,
        "Expected at least 2 errors, got {}",
        errors.len()
    );
}

#[test]
fn test_error_message_formatting() {
    let type_desc = TypeDescriptor::String(StringConstraints {
        min_length: Some(5),
        ..Default::default()
    });

    let value = Value::String("hi".to_string());
    let result = validate(&value, &type_desc);
    assert!(result.is_err());

    let errors = result.unwrap_err();
    let error = &errors.errors[0];

    // Error message should be descriptive
    assert!(!error.message.is_empty());
    // Should mention the constraint violation
    assert!(
        error.message.to_lowercase().contains("length")
            || error.message.to_lowercase().contains("min")
            || error.message.to_lowercase().contains("short")
            || error.message.contains("5")
            || error.message.contains("2")
    );
}

#[test]
fn test_error_context_preservation() {
    let mut ctx = ValidationContext::new();
    ctx.push("body");
    ctx.push("user");
    ctx.push("profile");

    assert_eq!(ctx.current_path(), "body.user.profile");
    assert_eq!(ctx.location(), "body");
    assert_eq!(ctx.field(), "user.profile");

    ctx.pop();
    assert_eq!(ctx.current_path(), "body.user");
    assert_eq!(ctx.field(), "user");
}

#[test]
fn test_error_to_display() {
    let error = ValidationError::new(
        "body".to_string(),
        "user.email".to_string(),
        "Invalid email format".to_string(),
        ErrorType::FormatError,
    );

    let display = format!("{}", error);
    assert!(display.contains("body"));
    assert!(display.contains("user.email"));
    assert!(display.contains("Invalid email format"));
}

#[test]
fn test_custom_error_messages() {
    use schema::custom_validators::field_error;

    let errors = field_error(
        "password",
        "Password must contain at least one uppercase letter",
    );
    assert_eq!(errors.len(), 1);

    let error = &errors.errors[0];
    assert_eq!(error.field, "password");
    assert!(error.message.contains("uppercase"));
}

#[test]
fn test_error_type_classification() {
    // Type error
    let type_error = ValidationError::type_error(
        "body".to_string(),
        "age".to_string(),
        "Expected integer, got string".to_string(),
    );
    assert_eq!(type_error.error_type, ErrorType::TypeError);

    // Value error
    let value_error = ValidationError::value_error(
        "body".to_string(),
        "count".to_string(),
        "Value must be positive".to_string(),
    );
    assert_eq!(value_error.error_type, ErrorType::ValueError);

    // Missing error
    let missing_error = ValidationError::missing_error("body".to_string(), "name".to_string());
    assert_eq!(missing_error.error_type, ErrorType::Missing);
}

// ============================================================================
// Error Aggregation
// ============================================================================

#[test]
fn test_validation_errors_merge() {
    let mut errors1 = ValidationErrors::new();
    errors1.add(ValidationError::type_error(
        "body".to_string(),
        "field1".to_string(),
        "Error 1".to_string(),
    ));

    let mut errors2 = ValidationErrors::new();
    errors2.add(ValidationError::value_error(
        "body".to_string(),
        "field2".to_string(),
        "Error 2".to_string(),
    ));

    errors1.merge(errors2);
    assert_eq!(errors1.len(), 2);
}

#[test]
fn test_validation_errors_into_result() {
    // Empty errors -> Ok
    let empty = ValidationErrors::new();
    assert!(empty.into_result().is_ok());

    // Non-empty errors -> Err
    let mut errors = ValidationErrors::new();
    errors.add(ValidationError::missing_error(
        "body".to_string(),
        "x".to_string(),
    ));
    assert!(errors.into_result().is_err());
}

// ============================================================================
// Error Cases - Multi-depth and Root Level
// ============================================================================

#[test]
fn test_error_multiple_depths_same_object() {
    // Errors at different nesting levels in the same object
    let type_desc = TypeDescriptor::Object {
        fields: vec![
            FieldDescriptor::new("name", TypeDescriptor::String(Default::default())),
            FieldDescriptor::new(
                "profile",
                TypeDescriptor::Object {
                    fields: vec![
                        FieldDescriptor::new("age", TypeDescriptor::Int64(Default::default())),
                        FieldDescriptor::new(
                            "contact",
                            TypeDescriptor::Object {
                                fields: vec![FieldDescriptor::new("email", TypeDescriptor::Email)],
                                additional: None,
                            },
                        ),
                    ],
                    additional: None,
                },
            ),
        ],
        additional: None,
    };

    // Errors at root (name), depth 1 (age), and depth 2 (email)
    let value = Value::Object(vec![
        ("name".to_string(), Value::Int(123)), // wrong type at root
        (
            "profile".to_string(),
            Value::Object(vec![
                ("age".to_string(), Value::String("not-int".to_string())), // wrong type at depth 1
                (
                    "contact".to_string(),
                    Value::Object(vec![
                        ("email".to_string(), Value::String("invalid".to_string())), // invalid format at depth 2
                    ]),
                ),
            ]),
        ),
    ]);

    let result = validate(&value, &type_desc);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    // Should capture errors at multiple depths
    assert!(
        errors.len() >= 2,
        "Expected multiple errors at different depths"
    );
}

#[test]
fn test_error_invalid_type_at_root() {
    // Expect object but get non-object at root
    let type_desc = TypeDescriptor::Object {
        fields: vec![FieldDescriptor::new(
            "name",
            TypeDescriptor::String(Default::default()),
        )],
        additional: None,
    };

    // Pass a string instead of object
    let value = Value::String("not an object".to_string());
    let result = validate(&value, &type_desc);
    assert!(result.is_err());

    let errors = result.unwrap_err();
    assert!(!errors.is_empty());
    // Root-level error should have empty or minimal path
    let error = &errors.errors[0];
    assert!(error.error_type == ErrorType::TypeError);
}

#[test]
fn test_error_array_of_objects_multiple_failures() {
    // Array of objects with errors in different indices
    let type_desc = TypeDescriptor::List {
        items: Box::new(TypeDescriptor::Object {
            fields: vec![FieldDescriptor::new(
                "value",
                TypeDescriptor::Int64(Default::default()),
            )],
            additional: None,
        }),
        constraints: Default::default(),
    };

    let value = Value::List(vec![
        Value::Object(vec![("value".to_string(), Value::Int(1))]), // valid
        Value::Object(vec![(
            "value".to_string(),
            Value::String("bad".to_string()),
        )]), // error at [1]
        Value::Object(vec![("value".to_string(), Value::Int(3))]), // valid
        Value::Object(vec![("value".to_string(), Value::Bool(true))]), // error at [3]
    ]);

    let result = validate(&value, &type_desc);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors.len() >= 2, "Expected errors at indices 1 and 3");
}

#[test]
fn test_error_missing_required_nested_field() {
    // Missing required field inside nested object
    let type_desc = TypeDescriptor::Object {
        fields: vec![FieldDescriptor::new(
            "user",
            TypeDescriptor::Object {
                fields: vec![
                    FieldDescriptor::new("name", TypeDescriptor::String(Default::default())),
                    FieldDescriptor::new("email", TypeDescriptor::Email), // required
                ],
                additional: None,
            },
        )],
        additional: None,
    };

    // email is missing inside user
    let value = Value::Object(vec![(
        "user".to_string(),
        Value::Object(vec![(
            "name".to_string(),
            Value::String("Alice".to_string()),
        )]),
    )]);

    let result = validate(&value, &type_desc);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    // Error path should point to nested field
    let has_nested_error = errors.errors.iter().any(|e| {
        e.field.contains("email")
            || e.location.contains("user")
            || e.error_type == ErrorType::Missing
    });
    assert!(has_nested_error);
}

// ============================================================================
// Edge Cases - Special Characters, Deep Nesting, Null
// ============================================================================

#[test]
fn test_edge_case_special_characters_in_field_names() {
    // Field names with dots, brackets, unicode
    let type_desc = TypeDescriptor::Object {
        fields: vec![
            FieldDescriptor::new("user.name", TypeDescriptor::String(Default::default())),
            FieldDescriptor::new("items[0]", TypeDescriptor::Int64(Default::default())),
            FieldDescriptor::new("名前", TypeDescriptor::String(Default::default())), // Japanese
        ],
        additional: None,
    };

    let value = Value::Object(vec![
        ("user.name".to_string(), Value::Int(123)), // wrong type
        ("items[0]".to_string(), Value::String("bad".to_string())), // wrong type
        ("名前".to_string(), Value::Int(456)),      // wrong type
    ]);

    let result = validate(&value, &type_desc);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors.len() >= 3);
}

#[test]
fn test_edge_case_very_deep_nesting() {
    // Create 15 levels of nesting
    fn create_nested_type(depth: usize) -> TypeDescriptor {
        if depth == 0 {
            TypeDescriptor::Int64(Default::default())
        } else {
            TypeDescriptor::Object {
                fields: vec![FieldDescriptor::new(
                    format!("level{}", depth),
                    create_nested_type(depth - 1),
                )],
                additional: None,
            }
        }
    }

    fn create_nested_value(depth: usize, leaf: Value) -> Value {
        if depth == 0 {
            leaf
        } else {
            Value::Object(vec![(
                format!("level{}", depth),
                create_nested_value(depth - 1, leaf),
            )])
        }
    }

    let type_desc = create_nested_type(15);
    let bad_value = create_nested_value(15, Value::String("not-an-int".to_string()));

    let result = validate(&bad_value, &type_desc);
    assert!(result.is_err());
    // Should handle deep nesting without stack overflow
    let errors = result.unwrap_err();
    assert!(!errors.is_empty());
}

#[test]
fn test_edge_case_null_in_required_field() {
    let type_desc = TypeDescriptor::Object {
        fields: vec![FieldDescriptor::new(
            "name",
            TypeDescriptor::String(Default::default()),
        )],
        additional: None,
    };

    // Null for required string field
    let value = Value::Object(vec![("name".to_string(), Value::Null)]);

    let result = validate(&value, &type_desc);
    // Should fail - null not allowed for required field
    assert!(result.is_err());
}

#[test]
fn test_edge_case_empty_object() {
    let type_desc = TypeDescriptor::Object {
        fields: vec![
            FieldDescriptor::new("name", TypeDescriptor::String(Default::default())),
            FieldDescriptor::new("age", TypeDescriptor::Int64(Default::default())),
        ],
        additional: None,
    };

    // Empty object - missing required fields
    let value = Value::Object(vec![]);

    let result = validate(&value, &type_desc);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    // Should have errors for missing name and age
    assert!(errors.len() >= 2);
}

#[test]
fn test_edge_case_empty_list() {
    let type_desc = TypeDescriptor::List {
        items: Box::new(TypeDescriptor::Int64(Default::default())),
        constraints: ListConstraints {
            min_items: Some(1), // at least 1 item required
            ..Default::default()
        },
    };

    // Empty list violates min_items
    let value = Value::List(vec![]);

    let result = validate(&value, &type_desc);
    assert!(result.is_err());
}

#[test]
fn test_edge_case_empty_string() {
    let type_desc = TypeDescriptor::String(StringConstraints {
        min_length: Some(1),
        ..Default::default()
    });

    let value = Value::String("".to_string());

    let result = validate(&value, &type_desc);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(!errors.is_empty());
}

#[test]
fn test_edge_case_numeric_boundaries() {
    let type_desc = TypeDescriptor::Int64(NumericConstraints {
        minimum: Some(0),
        maximum: Some(100),
        ..Default::default()
    });

    // Boundary values
    assert!(validate(&Value::Int(0), &type_desc).is_ok()); // min boundary
    assert!(validate(&Value::Int(100), &type_desc).is_ok()); // max boundary
    assert!(validate(&Value::Int(-1), &type_desc).is_err()); // below min
    assert!(validate(&Value::Int(101), &type_desc).is_err()); // above max

    // Extreme values
    assert!(validate(&Value::Int(i64::MIN), &type_desc).is_err());
    assert!(validate(&Value::Int(i64::MAX), &type_desc).is_err());
}
