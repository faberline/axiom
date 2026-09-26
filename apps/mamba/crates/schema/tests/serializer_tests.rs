//! Serializer Integration Tests
//!
//! Tests for serialization functionality including model_dump,
//! computed fields, exclude/include, and round-trip validation.

use schema::computed::*;
use schema::constraints::*;
use schema::serializers::*;
use schema::types::*;
use schema::validate;

// ============================================================================
// Basic Serialization Tests
// ============================================================================

#[test]
fn test_model_dump_basic() {
    let collection = SerializerCollection::new();

    // No serializers - should pass through unchanged
    let ctx = SerializerContext::new();
    let model = Value::Object(vec![
        ("name".to_string(), Value::String("Alice".to_string())),
        ("age".to_string(), Value::Int(30)),
    ]);

    let result = collection.serialize_model(&model, &ctx);
    assert_eq!(result, model);
}

#[test]
fn test_model_dump_with_field_serializer() {
    let mut collection = SerializerCollection::new();

    // Add serializer that uppercases the name
    collection.add_field_serializer(FnFieldSerializer::new("name", |value, _ctx| {
        if let Value::String(s) = value {
            Value::String(s.to_uppercase())
        } else {
            value.clone()
        }
    }));

    let ctx = SerializerContext::new();
    let model = Value::Object(vec![
        ("name".to_string(), Value::String("alice".to_string())),
        ("age".to_string(), Value::Int(30)),
    ]);

    let result = collection.serialize_model(&model, &ctx);

    if let Value::Object(fields) = result {
        let name = fields.iter().find(|(k, _)| k == "name").unwrap();
        assert_eq!(name.1, Value::String("ALICE".to_string()));
    } else {
        panic!("Expected object");
    }
}

// ============================================================================
// Exclude/Include Fields
// ============================================================================

#[test]
fn test_exclude_fields() {
    let collection = SerializerCollection::new();

    let ctx =
        SerializerContext::new().with_exclude(vec!["password".to_string(), "secret".to_string()]);

    let model = Value::Object(vec![
        ("username".to_string(), Value::String("alice".to_string())),
        (
            "password".to_string(),
            Value::String("secret123".to_string()),
        ),
        (
            "email".to_string(),
            Value::String("alice@example.com".to_string()),
        ),
        ("secret".to_string(), Value::String("hidden".to_string())),
    ]);

    let result = collection.serialize_model(&model, &ctx);

    if let Value::Object(fields) = result {
        assert_eq!(fields.len(), 2);
        assert!(fields.iter().any(|(k, _)| k == "username"));
        assert!(fields.iter().any(|(k, _)| k == "email"));
        assert!(!fields.iter().any(|(k, _)| k == "password"));
        assert!(!fields.iter().any(|(k, _)| k == "secret"));
    } else {
        panic!("Expected object");
    }
}

#[test]
fn test_include_fields() {
    let collection = SerializerCollection::new();

    let ctx = SerializerContext::new().with_include(vec!["id".to_string(), "name".to_string()]);

    let model = Value::Object(vec![
        ("id".to_string(), Value::Int(1)),
        ("name".to_string(), Value::String("Alice".to_string())),
        (
            "email".to_string(),
            Value::String("alice@example.com".to_string()),
        ),
        ("age".to_string(), Value::Int(30)),
    ]);

    let result = collection.serialize_model(&model, &ctx);

    if let Value::Object(fields) = result {
        assert_eq!(fields.len(), 2);
        assert!(fields.iter().any(|(k, _)| k == "id"));
        assert!(fields.iter().any(|(k, _)| k == "name"));
    } else {
        panic!("Expected object");
    }
}

#[test]
fn test_exclude_none_values() {
    let collection = SerializerCollection::new();

    // Default: exclude None
    let ctx = SerializerContext::new().with_include_none(false);

    let model = Value::Object(vec![
        ("name".to_string(), Value::String("Alice".to_string())),
        ("nickname".to_string(), Value::Null),
        ("age".to_string(), Value::Int(30)),
    ]);

    let result = collection.serialize_model(&model, &ctx);

    if let Value::Object(fields) = result {
        assert_eq!(fields.len(), 2);
        assert!(!fields.iter().any(|(k, _)| k == "nickname"));
    } else {
        panic!("Expected object");
    }

    // Include None
    let ctx_with_none = SerializerContext::new().with_include_none(true);
    let result_with_none = collection.serialize_model(&model, &ctx_with_none);

    if let Value::Object(fields) = result_with_none {
        assert_eq!(fields.len(), 3);
        assert!(fields.iter().any(|(k, _)| k == "nickname"));
    } else {
        panic!("Expected object");
    }
}

// ============================================================================
// Computed Fields Serialization
// ============================================================================

#[test]
fn test_computed_field_serialization() {
    // Create computed field that combines first_name and last_name
    let full_name_computed = FnComputedField::new("full_name", |fields| {
        let first = get_string_field(fields, "first_name").unwrap_or_default();
        let last = get_string_field(fields, "last_name").unwrap_or_default();
        Value::String(format!("{} {}", first, last))
    });

    let mut collection = ComputedFieldCollection::new();
    collection.add(full_name_computed);

    let model = Value::Object(vec![
        ("first_name".to_string(), Value::String("Alice".to_string())),
        ("last_name".to_string(), Value::String("Smith".to_string())),
    ]);

    let result = collection.compute_values(&model);
    assert!(result.contains_key("full_name"));
    assert_eq!(
        result.get("full_name").unwrap(),
        &Value::String("Alice Smith".to_string())
    );
}

#[test]
fn test_computed_field_with_calculation() {
    // Computed field that calculates total from price * quantity
    let total_computed = FnComputedField::new("total", |fields| {
        let price = get_float_field(fields, "price").unwrap_or(0.0);
        let quantity = get_int_field(fields, "quantity").unwrap_or(0);
        Value::Float(price * quantity as f64)
    });

    let mut collection = ComputedFieldCollection::new();
    collection.add(total_computed);

    let model = Value::Object(vec![
        ("price".to_string(), Value::Float(9.99)),
        ("quantity".to_string(), Value::Int(3)),
    ]);

    let result = collection.compute_values(&model);
    assert!(result.contains_key("total"));

    if let Value::Float(total) = result.get("total").unwrap() {
        assert!((total - 29.97).abs() < 0.01);
    } else {
        panic!("Expected float");
    }
}

// ============================================================================
// Mask Serializer Tests
// ============================================================================

#[test]
fn test_mask_serializer_credit_card() {
    let mut collection = SerializerCollection::new();
    collection.add_field_serializer(MaskSerializer::new("card_number", '*', 4));

    let ctx = SerializerContext::new();
    let model = Value::Object(vec![(
        "card_number".to_string(),
        Value::String("4111111111111111".to_string()),
    )]);

    let result = collection.serialize_model(&model, &ctx);

    if let Value::Object(fields) = result {
        let card = fields.iter().find(|(k, _)| k == "card_number").unwrap();
        assert_eq!(card.1, Value::String("************1111".to_string()));
    } else {
        panic!("Expected object");
    }
}

#[test]
fn test_mask_serializer_short_value() {
    let mut collection = SerializerCollection::new();
    collection.add_field_serializer(MaskSerializer::new("pin", '*', 2));

    let ctx = SerializerContext::new();

    // Value shorter than visible_chars
    let model = Value::Object(vec![("pin".to_string(), Value::String("1".to_string()))]);

    let result = collection.serialize_model(&model, &ctx);

    if let Value::Object(fields) = result {
        let pin = fields.iter().find(|(k, _)| k == "pin").unwrap();
        // Entire value should be masked
        assert_eq!(pin.1, Value::String("*".to_string()));
    } else {
        panic!("Expected object");
    }
}

// ============================================================================
// Round-Trip Validation
// ============================================================================

#[test]
fn test_round_trip_validation() {
    // Define type
    let type_desc = TypeDescriptor::Object {
        fields: vec![
            FieldDescriptor::new("name", TypeDescriptor::String(Default::default())),
            FieldDescriptor::new("age", TypeDescriptor::Int64(Default::default())),
            FieldDescriptor::new("email", TypeDescriptor::Email),
        ],
        additional: None,
    };

    // Create value
    let original = Value::Object(vec![
        ("name".to_string(), Value::String("Alice".to_string())),
        ("age".to_string(), Value::Int(30)),
        (
            "email".to_string(),
            Value::String("alice@example.com".to_string()),
        ),
    ]);

    // Validate
    assert!(validate(&original, &type_desc).is_ok());

    // Serialize (no-op for now)
    let collection = SerializerCollection::new();
    let ctx = SerializerContext::new();
    let serialized = collection.serialize_model(&original, &ctx);

    // Validate again
    assert!(validate(&serialized, &type_desc).is_ok());

    // Values should be equal
    assert_eq!(original, serialized);
}

#[test]
fn test_round_trip_with_transformation() {
    let type_desc = TypeDescriptor::Object {
        fields: vec![FieldDescriptor::new(
            "email",
            TypeDescriptor::String(Default::default()),
        )],
        additional: None,
    };

    // Original with mixed case
    let original = Value::Object(vec![(
        "email".to_string(),
        Value::String("USER@EXAMPLE.COM".to_string()),
    )]);

    // Validate original
    assert!(validate(&original, &type_desc).is_ok());

    // Serialize with lowercase transformation
    let mut collection = SerializerCollection::new();
    collection.add_field_serializer(FnFieldSerializer::new("email", |value, _ctx| {
        if let Value::String(s) = value {
            Value::String(s.to_lowercase())
        } else {
            value.clone()
        }
    }));

    let ctx = SerializerContext::new();
    let serialized = collection.serialize_model(&original, &ctx);

    // Validate serialized
    assert!(validate(&serialized, &type_desc).is_ok());

    // Email should be lowercased
    if let Value::Object(fields) = serialized {
        let email = fields.iter().find(|(k, _)| k == "email").unwrap();
        assert_eq!(email.1, Value::String("user@example.com".to_string()));
    }
}

// ============================================================================
// Serializer Mode Tests
// ============================================================================

#[test]
fn test_serializer_mode_json() {
    let mut collection = SerializerCollection::new();

    // JSON-only serializer
    collection.add_field_serializer(
        FnFieldSerializer::new("timestamp", |value, _ctx| {
            // Format timestamp for JSON
            if let Value::Int(ts) = value {
                Value::String(format!("{}Z", ts))
            } else {
                value.clone()
            }
        })
        .mode(SerializerMode::Json),
    );

    let model = Value::Object(vec![("timestamp".to_string(), Value::Int(1705680000))]);

    // JSON context - should apply serializer
    let json_ctx = SerializerContext::json();
    let json_result = collection.serialize_model(&model, &json_ctx);

    if let Value::Object(fields) = json_result {
        let ts = fields.iter().find(|(k, _)| k == "timestamp").unwrap();
        assert_eq!(ts.1, Value::String("1705680000Z".to_string()));
    }

    // Python context - should NOT apply serializer
    let python_ctx = SerializerContext::python();
    let python_result = collection.serialize_model(&model, &python_ctx);

    if let Value::Object(fields) = python_result {
        let ts = fields.iter().find(|(k, _)| k == "timestamp").unwrap();
        assert_eq!(ts.1, Value::Int(1705680000));
    }
}

#[test]
fn test_model_serializer() {
    let mut collection = SerializerCollection::new();

    // Model serializer that adds metadata
    collection.add_model_serializer(FnModelSerializer::new(|value, _ctx| {
        if let Value::Object(mut fields) = value.clone() {
            fields.push(("_serialized".to_string(), Value::Bool(true)));
            Value::Object(fields)
        } else {
            value.clone()
        }
    }));

    let ctx = SerializerContext::new();
    let model = Value::Object(vec![("data".to_string(), Value::Int(42))]);

    let result = collection.serialize_model(&model, &ctx);

    if let Value::Object(fields) = result {
        assert!(fields.iter().any(|(k, _)| k == "_serialized"));
    } else {
        panic!("Expected object");
    }
}

// ============================================================================
// Error Cases - Wrong Types, Missing Fields
// ============================================================================

#[test]
fn test_serializer_wrong_input_type_passthrough() {
    // Serializer expecting string but getting int - should handle gracefully
    let mut collection = SerializerCollection::new();

    collection.add_field_serializer(FnFieldSerializer::new("name", |value, _ctx| {
        if let Value::String(s) = value {
            Value::String(s.to_uppercase())
        } else {
            // Passthrough if not expected type
            value.clone()
        }
    }));

    let ctx = SerializerContext::new();
    let model = Value::Object(vec![("name".to_string(), Value::Int(123))]); // Int instead of String

    let result = collection.serialize_model(&model, &ctx);

    // Should pass through unchanged (no panic)
    if let Value::Object(fields) = result {
        let name = fields.iter().find(|(k, _)| k == "name").unwrap();
        assert_eq!(name.1, Value::Int(123)); // Unchanged
    } else {
        panic!("Expected object");
    }
}

#[test]
fn test_serializer_for_missing_field() {
    // Serializer for field that doesn't exist in model
    let mut collection = SerializerCollection::new();

    collection.add_field_serializer(FnFieldSerializer::new("nonexistent", |_value, _ctx| {
        Value::String("transformed".to_string())
    }));

    let ctx = SerializerContext::new();
    let model = Value::Object(vec![(
        "name".to_string(),
        Value::String("Alice".to_string()),
    )]);

    let result = collection.serialize_model(&model, &ctx);

    // Should complete without error, nonexistent field not added
    if let Value::Object(fields) = result {
        assert_eq!(fields.len(), 1);
        assert!(fields.iter().any(|(k, _)| k == "name"));
        assert!(!fields.iter().any(|(k, _)| k == "nonexistent"));
    } else {
        panic!("Expected object");
    }
}

#[test]
fn test_model_serializer_non_object_input() {
    // Model serializer receiving non-object
    let mut collection = SerializerCollection::new();

    collection.add_model_serializer(FnModelSerializer::new(|value, _ctx| {
        match value {
            Value::Object(_) => {
                let mut fields = if let Value::Object(f) = value.clone() {
                    f
                } else {
                    vec![]
                };
                fields.push(("_processed".to_string(), Value::Bool(true)));
                Value::Object(fields)
            }
            // For non-object, just pass through
            other => other.clone(),
        }
    }));

    let ctx = SerializerContext::new();

    // Non-object input - should pass through
    let result = collection.serialize_model(&Value::String("not object".to_string()), &ctx);
    assert_eq!(result, Value::String("not object".to_string()));

    let result = collection.serialize_model(&Value::Int(42), &ctx);
    assert_eq!(result, Value::Int(42));
}

#[test]
fn test_exclude_nonexistent_field() {
    // Exclude fields that don't exist - should not error
    let collection = SerializerCollection::new();

    let ctx = SerializerContext::new()
        .with_exclude(vec!["nonexistent1".to_string(), "nonexistent2".to_string()]);

    let model = Value::Object(vec![
        ("name".to_string(), Value::String("Alice".to_string())),
        ("age".to_string(), Value::Int(30)),
    ]);

    let result = collection.serialize_model(&model, &ctx);

    // Should complete without error, all fields present
    if let Value::Object(fields) = result {
        assert_eq!(fields.len(), 2);
    } else {
        panic!("Expected object");
    }
}

#[test]
fn test_include_nonexistent_field() {
    // Include fields that don't exist - should not error
    let collection = SerializerCollection::new();

    let ctx =
        SerializerContext::new().with_include(vec!["name".to_string(), "nonexistent".to_string()]);

    let model = Value::Object(vec![
        ("name".to_string(), Value::String("Alice".to_string())),
        ("age".to_string(), Value::Int(30)),
    ]);

    let result = collection.serialize_model(&model, &ctx);

    // Should only include "name" (nonexistent is ignored)
    if let Value::Object(fields) = result {
        assert_eq!(fields.len(), 1);
        assert!(fields.iter().any(|(k, _)| k == "name"));
    } else {
        panic!("Expected object");
    }
}

// ============================================================================
// Edge Cases - Include+Exclude, Empty, Null, Special Values
// ============================================================================

#[test]
fn test_include_and_exclude_together() {
    // When both include and exclude are specified
    let collection = SerializerCollection::new();

    // Include takes precedence - only include specified fields, then apply exclude
    let ctx = SerializerContext::new()
        .with_include(vec!["a".to_string(), "b".to_string(), "c".to_string()])
        .with_exclude(vec!["b".to_string()]);

    let model = Value::Object(vec![
        ("a".to_string(), Value::Int(1)),
        ("b".to_string(), Value::Int(2)),
        ("c".to_string(), Value::Int(3)),
        ("d".to_string(), Value::Int(4)),
    ]);

    let result = collection.serialize_model(&model, &ctx);

    if let Value::Object(fields) = result {
        // Should include a, c (from include list) but exclude b
        // d is not in include list so also excluded
        assert!(fields.iter().any(|(k, _)| k == "a"));
        assert!(!fields.iter().any(|(k, _)| k == "b")); // Excluded
        assert!(fields.iter().any(|(k, _)| k == "c"));
        assert!(!fields.iter().any(|(k, _)| k == "d")); // Not in include
    } else {
        panic!("Expected object");
    }
}

#[test]
fn test_empty_include_exclude_lists() {
    // Empty include/exclude should behave like no filters
    let collection = SerializerCollection::new();

    let ctx_empty_include = SerializerContext::new().with_include(vec![]);
    let ctx_empty_exclude = SerializerContext::new().with_exclude(vec![]);

    let model = Value::Object(vec![
        ("a".to_string(), Value::Int(1)),
        ("b".to_string(), Value::Int(2)),
    ]);

    // Empty include - behavior depends on implementation
    // Some libs: empty include = include nothing, others: empty include = include all
    let result_include = collection.serialize_model(&model, &ctx_empty_include);
    // Just check it doesn't crash
    assert!(matches!(result_include, Value::Object(_)));

    // Empty exclude - should include all
    let result_exclude = collection.serialize_model(&model, &ctx_empty_exclude);
    if let Value::Object(fields) = result_exclude {
        assert_eq!(fields.len(), 2);
    }
}

#[test]
fn test_include_none_with_nested_null() {
    // Nested nulls with include_none setting
    let collection = SerializerCollection::new();

    let model = Value::Object(vec![
        ("name".to_string(), Value::String("Alice".to_string())),
        ("profile".to_string(), Value::Null), // Null nested object
        (
            "settings".to_string(),
            Value::Object(vec![
                ("theme".to_string(), Value::String("dark".to_string())),
                ("notifications".to_string(), Value::Null), // Null inside object
            ]),
        ),
    ]);

    // Exclude none
    let ctx_no_none = SerializerContext::new().with_include_none(false);
    let result = collection.serialize_model(&model, &ctx_no_none);

    if let Value::Object(fields) = result {
        // Top-level null (profile) should be excluded
        assert!(!fields.iter().any(|(k, _)| k == "profile"));
        // Nested null (notifications) - depends on whether serializer is recursive
    }

    // Include none
    let ctx_with_none = SerializerContext::new().with_include_none(true);
    let result = collection.serialize_model(&model, &ctx_with_none);

    if let Value::Object(fields) = result {
        assert!(fields.iter().any(|(k, _)| k == "profile"));
    }
}

#[test]
fn test_list_with_null_values() {
    // List containing null values
    let collection = SerializerCollection::new();

    let model = Value::Object(vec![(
        "items".to_string(),
        Value::List(vec![Value::Int(1), Value::Null, Value::Int(3), Value::Null]),
    )]);

    let ctx = SerializerContext::new();
    let result = collection.serialize_model(&model, &ctx);

    // List nulls should be preserved (lists aren't filtered by include_none)
    if let Value::Object(fields) = result {
        let items = fields.iter().find(|(k, _)| k == "items").unwrap();
        if let Value::List(list) = &items.1 {
            assert_eq!(list.len(), 4);
            assert_eq!(list[1], Value::Null);
            assert_eq!(list[3], Value::Null);
        }
    }
}

#[test]
fn test_mask_serializer_empty_string() {
    let mut collection = SerializerCollection::new();
    collection.add_field_serializer(MaskSerializer::new("secret", '*', 4));

    let ctx = SerializerContext::new();
    let model = Value::Object(vec![("secret".to_string(), Value::String("".to_string()))]);

    let result = collection.serialize_model(&model, &ctx);

    if let Value::Object(fields) = result {
        let secret = fields.iter().find(|(k, _)| k == "secret").unwrap();
        // Empty string masked should still be empty or minimal
        assert_eq!(secret.1, Value::String("".to_string()));
    }
}

#[test]
fn test_mask_serializer_visible_zero() {
    // Mask with visible_chars = 0 (hide everything)
    let mut collection = SerializerCollection::new();
    collection.add_field_serializer(MaskSerializer::new("token", '*', 0));

    let ctx = SerializerContext::new();
    let model = Value::Object(vec![(
        "token".to_string(),
        Value::String("secret123".to_string()),
    )]);

    let result = collection.serialize_model(&model, &ctx);

    if let Value::Object(fields) = result {
        let token = fields.iter().find(|(k, _)| k == "token").unwrap();
        // Everything should be masked
        if let Value::String(s) = &token.1 {
            assert!(!s.contains("secret"));
            assert!(!s.contains("123"));
            assert!(s.chars().all(|c| c == '*'));
        }
    }
}

#[test]
fn test_computed_field_missing_source_fields() {
    // Computed field when source fields are missing
    let computed = FnComputedField::new("full_name", |fields| {
        let first = get_string_field(fields, "first_name").unwrap_or_default();
        let last = get_string_field(fields, "last_name").unwrap_or_default();
        if first.is_empty() && last.is_empty() {
            Value::Null
        } else {
            Value::String(format!("{} {}", first, last).trim().to_string())
        }
    });

    let mut collection = ComputedFieldCollection::new();
    collection.add(computed);

    // No source fields
    let model = Value::Object(vec![("other".to_string(), Value::Int(1))]);
    let result = collection.compute_values(&model);

    // Should handle gracefully (return empty string or null)
    assert!(result.contains_key("full_name"));
}

#[test]
fn test_computed_field_non_object_input() {
    // Computed field with non-object input
    let computed = FnComputedField::new("computed", |fields| {
        // Safely handle non-object
        match fields {
            Value::Object(_) => Value::String("from object".to_string()),
            _ => Value::String("not an object".to_string()),
        }
    });

    let mut collection = ComputedFieldCollection::new();
    collection.add(computed);

    // Non-object input
    let result = collection.compute_values(&Value::List(vec![]));

    // Should handle without panic
    assert!(result.contains_key("computed"));
}

#[test]
fn test_serializer_with_unicode_field_names() {
    // Unicode field names
    let mut collection = SerializerCollection::new();

    collection.add_field_serializer(FnFieldSerializer::new("名前", |value, _ctx| {
        if let Value::String(s) = value {
            Value::String(format!("Hello, {}", s))
        } else {
            value.clone()
        }
    }));

    let ctx = SerializerContext::new();
    let model = Value::Object(vec![(
        "名前".to_string(),
        Value::String("田中".to_string()),
    )]);

    let result = collection.serialize_model(&model, &ctx);

    if let Value::Object(fields) = result {
        let name = fields.iter().find(|(k, _)| k == "名前").unwrap();
        assert_eq!(name.1, Value::String("Hello, 田中".to_string()));
    }
}

#[test]
fn test_serializer_preserves_field_order() {
    // Field order should be preserved through serialization
    let collection = SerializerCollection::new();
    let ctx = SerializerContext::new();

    let model = Value::Object(vec![
        ("z".to_string(), Value::Int(1)),
        ("a".to_string(), Value::Int(2)),
        ("m".to_string(), Value::Int(3)),
    ]);

    let result = collection.serialize_model(&model, &ctx);

    if let Value::Object(fields) = result {
        assert_eq!(fields[0].0, "z");
        assert_eq!(fields[1].0, "a");
        assert_eq!(fields[2].0, "m");
    }
}

#[test]
fn test_multiple_field_serializers_same_field() {
    // Multiple serializers for same field - should chain
    let mut collection = SerializerCollection::new();

    // First: trim
    collection.add_field_serializer(FnFieldSerializer::new("name", |value, _ctx| {
        if let Value::String(s) = value {
            Value::String(s.trim().to_string())
        } else {
            value.clone()
        }
    }));

    // Second: uppercase
    collection.add_field_serializer(FnFieldSerializer::new("name", |value, _ctx| {
        if let Value::String(s) = value {
            Value::String(s.to_uppercase())
        } else {
            value.clone()
        }
    }));

    let ctx = SerializerContext::new();
    let model = Value::Object(vec![(
        "name".to_string(),
        Value::String("  alice  ".to_string()),
    )]);

    let result = collection.serialize_model(&model, &ctx);

    if let Value::Object(fields) = result {
        let name = fields.iter().find(|(k, _)| k == "name").unwrap();
        // Should be both trimmed and uppercased
        assert_eq!(name.1, Value::String("ALICE".to_string()));
    }
}
