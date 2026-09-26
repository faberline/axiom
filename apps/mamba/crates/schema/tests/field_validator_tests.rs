//! Field Validator Modes Tests
//!
//! Tests for field-level validators with different modes (before/after/wrap),
//! similar to Pydantic's @field_validator.

use schema::custom_validators::*;
use schema::types::*;

// ============================================================================
// Field Validator Modes
// ============================================================================

#[test]
fn test_field_validator_before_mode() {
    // Before mode: runs before type coercion
    // Can transform raw input before it's validated
    let validator = FnFieldValidator::new("name", |value, _ctx| {
        // Trim whitespace from string before validation
        if let Value::String(s) = value {
            Ok(Value::String(s.trim().to_string()))
        } else {
            Ok(value.clone())
        }
    })
    .mode(ValidatorMode::Before);

    assert_eq!(validator.field_name(), "name");

    let ctx = ValidatorContext::new();
    let input = Value::String("  Alice  ".to_string());
    let result = FieldValidator::validate(&validator, &input, &ctx).unwrap();

    assert_eq!(result, Value::String("Alice".to_string()));
}

#[test]
fn test_field_validator_after_mode() {
    // After mode: runs after type coercion (default)
    let validator = FnFieldValidator::new("email", |value, _ctx| {
        // Validate email format after type coercion
        if let Value::String(s) = value {
            if !s.contains('@') {
                return Err(field_error("email", "Invalid email format"));
            }
            // Normalize to lowercase
            Ok(Value::String(s.to_lowercase()))
        } else {
            Err(field_error("email", "Expected string"))
        }
    })
    .mode(ValidatorMode::After);

    let ctx = ValidatorContext::new();

    // Valid email
    let valid = Value::String("USER@EXAMPLE.COM".to_string());
    let result = FieldValidator::validate(&validator, &valid, &ctx).unwrap();
    assert_eq!(result, Value::String("user@example.com".to_string()));

    // Invalid email
    let invalid = Value::String("not-an-email".to_string());
    assert!(FieldValidator::validate(&validator, &invalid, &ctx).is_err());
}

#[test]
fn test_field_validator_wrap_mode() {
    // Wrap mode: controls the entire field validation
    let validator = FnFieldValidator::new("quantity", |value, _ctx| {
        // Can decide whether to continue with validation or short-circuit
        if let Value::Int(n) = value {
            if *n < 0 {
                // Transform negative to 0
                Ok(Value::Int(0))
            } else if *n > 1000 {
                // Reject values over 1000
                Err(field_error("quantity", "Quantity cannot exceed 1000"))
            } else {
                Ok(value.clone())
            }
        } else {
            Err(field_error("quantity", "Expected integer"))
        }
    })
    .mode(ValidatorMode::Wrap);

    let ctx = ValidatorContext::new();

    // Normal value
    assert_eq!(
        FieldValidator::validate(&validator, &Value::Int(50), &ctx).unwrap(),
        Value::Int(50)
    );

    // Negative transformed to 0
    assert_eq!(
        FieldValidator::validate(&validator, &Value::Int(-10), &ctx).unwrap(),
        Value::Int(0)
    );

    // Over limit rejected
    assert!(FieldValidator::validate(&validator, &Value::Int(1001), &ctx).is_err());
}

#[test]
fn test_field_validator_error_message() {
    let validator = FnFieldValidator::new("age", |value, _ctx| {
        if let Value::Int(age) = value {
            if *age < 0 {
                return Err(field_error("age", "Age cannot be negative"));
            }
            if *age > 150 {
                return Err(field_error("age", "Age cannot exceed 150"));
            }
        }
        Ok(value.clone())
    });

    let ctx = ValidatorContext::new();

    // Valid
    assert!(FieldValidator::validate(&validator, &Value::Int(25), &ctx).is_ok());

    // Negative age
    let result = FieldValidator::validate(&validator, &Value::Int(-5), &ctx);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors.errors[0].message.contains("negative"));

    // Too old
    let result = FieldValidator::validate(&validator, &Value::Int(200), &ctx);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors.errors[0].message.contains("150"));
}

#[test]
fn test_field_validator_with_other_fields() {
    // Access other fields through context
    let validator = FnFieldValidator::new("end_time", |value, ctx| {
        if let Some(start) = ctx.get_field("start_time") {
            if let (Value::Int(end), Value::Int(start_val)) = (value, start) {
                if end <= start_val {
                    return Err(field_error("end_time", "End time must be after start time"));
                }
            }
        }
        Ok(value.clone())
    });

    let mut ctx = ValidatorContext::new();
    ctx.set_field("start_time", Value::Int(100));

    // Valid: end > start
    assert!(FieldValidator::validate(&validator, &Value::Int(200), &ctx).is_ok());

    // Invalid: end <= start
    assert!(FieldValidator::validate(&validator, &Value::Int(50), &ctx).is_err());
    assert!(FieldValidator::validate(&validator, &Value::Int(100), &ctx).is_err());
}

// ============================================================================
// Common Field Validation Patterns
// ============================================================================

#[test]
fn test_string_normalization() {
    let validator = FnFieldValidator::new("username", |value, _ctx| {
        if let Value::String(s) = value {
            // Normalize: trim, lowercase, replace spaces with underscores
            let normalized = s.trim().to_lowercase().replace(' ', "_");
            Ok(Value::String(normalized))
        } else {
            Err(field_error("username", "Expected string"))
        }
    })
    .mode(ValidatorMode::Before);

    let ctx = ValidatorContext::new();

    let input = Value::String("  John Doe  ".to_string());
    let result = FieldValidator::validate(&validator, &input, &ctx).unwrap();
    assert_eq!(result, Value::String("john_doe".to_string()));
}

#[test]
fn test_numeric_range_validation() {
    let validator = FnFieldValidator::new("percentage", |value, _ctx| {
        if let Value::Float(n) = value {
            if *n < 0.0 || *n > 100.0 {
                return Err(field_error(
                    "percentage",
                    "Percentage must be between 0 and 100",
                ));
            }
        }
        Ok(value.clone())
    });

    let ctx = ValidatorContext::new();

    assert!(FieldValidator::validate(&validator, &Value::Float(50.0), &ctx).is_ok());
    assert!(FieldValidator::validate(&validator, &Value::Float(0.0), &ctx).is_ok());
    assert!(FieldValidator::validate(&validator, &Value::Float(100.0), &ctx).is_ok());
    assert!(FieldValidator::validate(&validator, &Value::Float(-1.0), &ctx).is_err());
    assert!(FieldValidator::validate(&validator, &Value::Float(101.0), &ctx).is_err());
}

#[test]
fn test_list_element_validation() {
    let validator = FnFieldValidator::new("tags", |value, _ctx| {
        if let Value::List(items) = value {
            // Validate each tag is non-empty
            for (i, item) in items.iter().enumerate() {
                if let Value::String(s) = item {
                    if s.is_empty() {
                        return Err(field_error(
                            "tags",
                            format!("Tag at index {} cannot be empty", i),
                        ));
                    }
                }
            }
        }
        Ok(value.clone())
    });

    let ctx = ValidatorContext::new();

    // Valid
    let valid = Value::List(vec![
        Value::String("rust".to_string()),
        Value::String("python".to_string()),
    ]);
    assert!(FieldValidator::validate(&validator, &valid, &ctx).is_ok());

    // Invalid: empty tag
    let invalid = Value::List(vec![
        Value::String("rust".to_string()),
        Value::String("".to_string()),
    ]);
    assert!(FieldValidator::validate(&validator, &invalid, &ctx).is_err());
}

// ============================================================================
// Validator Collection with Field Validators
// ============================================================================

#[test]
fn test_field_validators_in_collection() {
    let mut collection = ValidatorCollection::new();

    // Add multiple validators for different fields
    collection.add_field_validator(FnFieldValidator::new("name", |value, _ctx| {
        if let Value::String(s) = value {
            if s.is_empty() {
                return Err(field_error("name", "Name cannot be empty"));
            }
        }
        Ok(value.clone())
    }));

    collection.add_field_validator(FnFieldValidator::new("age", |value, _ctx| {
        if let Value::Int(n) = value {
            if *n < 0 {
                return Err(field_error("age", "Age cannot be negative"));
            }
        }
        Ok(value.clone())
    }));

    let ctx = ValidatorContext::new();

    // Test name validation
    assert!(collection
        .run_field_validators(
            "name",
            &Value::String("Alice".to_string()),
            &ctx,
            ValidatorMode::After
        )
        .is_ok());
    assert!(collection
        .run_field_validators(
            "name",
            &Value::String("".to_string()),
            &ctx,
            ValidatorMode::After
        )
        .is_err());

    // Test age validation
    assert!(collection
        .run_field_validators("age", &Value::Int(25), &ctx, ValidatorMode::After)
        .is_ok());
    assert!(collection
        .run_field_validators("age", &Value::Int(-5), &ctx, ValidatorMode::After)
        .is_err());

    // Test unknown field (should pass through)
    assert!(collection
        .run_field_validators("unknown", &Value::Bool(true), &ctx, ValidatorMode::After)
        .is_ok());
}

#[test]
fn test_multiple_validators_same_field() {
    let mut collection = ValidatorCollection::new();

    // First validator: trim
    collection.add_field_validator(
        FnFieldValidator::new("email", |value, _ctx| {
            if let Value::String(s) = value {
                Ok(Value::String(s.trim().to_string()))
            } else {
                Ok(value.clone())
            }
        })
        .mode(ValidatorMode::Before),
    );

    // Second validator: lowercase
    collection.add_field_validator(
        FnFieldValidator::new("email", |value, _ctx| {
            if let Value::String(s) = value {
                Ok(Value::String(s.to_lowercase()))
            } else {
                Ok(value.clone())
            }
        })
        .mode(ValidatorMode::Before),
    );

    let ctx = ValidatorContext::new();
    let input = Value::String("  USER@EXAMPLE.COM  ".to_string());

    let result = collection
        .run_field_validators("email", &input, &ctx, ValidatorMode::Before)
        .unwrap();
    assert_eq!(result, Value::String("user@example.com".to_string()));
}

#[test]
fn test_validator_mode_filtering() {
    let mut collection = ValidatorCollection::new();

    // Before mode validator
    collection.add_field_validator(
        FnFieldValidator::new("value", |value, _ctx| {
            if let Value::Int(n) = value {
                Ok(Value::Int(n + 1)) // Add 1
            } else {
                Ok(value.clone())
            }
        })
        .mode(ValidatorMode::Before),
    );

    // After mode validator
    collection.add_field_validator(
        FnFieldValidator::new("value", |value, _ctx| {
            if let Value::Int(n) = value {
                Ok(Value::Int(n * 2)) // Multiply by 2
            } else {
                Ok(value.clone())
            }
        })
        .mode(ValidatorMode::After),
    );

    let ctx = ValidatorContext::new();
    let input = Value::Int(5);

    // Only run Before validators: 5 + 1 = 6
    let after_before = collection
        .run_field_validators("value", &input, &ctx, ValidatorMode::Before)
        .unwrap();
    assert_eq!(after_before, Value::Int(6));

    // Only run After validators: 5 * 2 = 10
    let after_after = collection
        .run_field_validators("value", &input, &ctx, ValidatorMode::After)
        .unwrap();
    assert_eq!(after_after, Value::Int(10));
}

// ============================================================================
// Error Cases - Type Errors, Multiple Errors
// ============================================================================

#[test]
fn test_field_validator_wrong_type_before_mode() {
    // Before mode validator expecting string but getting int
    let validator = FnFieldValidator::new("name", |value, _ctx| {
        if let Value::String(s) = value {
            Ok(Value::String(s.to_uppercase()))
        } else {
            Err(field_error("name", "Expected string type"))
        }
    })
    .mode(ValidatorMode::Before);

    let ctx = ValidatorContext::new();

    // Pass wrong type
    let result = FieldValidator::validate(&validator, &Value::Int(123), &ctx);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors.errors[0].message.contains("string"));
}

#[test]
fn test_field_validator_wrong_type_after_mode() {
    // After mode validator expecting int but getting bool
    let validator = FnFieldValidator::new("count", |value, _ctx| {
        if let Value::Int(n) = value {
            Ok(Value::Int(n * 2))
        } else {
            Err(field_error("count", "Expected integer type"))
        }
    })
    .mode(ValidatorMode::After);

    let ctx = ValidatorContext::new();

    let result = FieldValidator::validate(&validator, &Value::Bool(true), &ctx);
    assert!(result.is_err());
}

#[test]
fn test_field_validator_wrong_type_wrap_mode() {
    // Wrap mode validator with strict type checking
    let validator = FnFieldValidator::new("data", |value, _ctx| match value {
        Value::Object(_) => Ok(value.clone()),
        _ => Err(field_error("data", "Expected object type")),
    })
    .mode(ValidatorMode::Wrap);

    let ctx = ValidatorContext::new();

    // Pass array instead of object
    let result = FieldValidator::validate(&validator, &Value::List(vec![]), &ctx);
    assert!(result.is_err());
}

#[test]
fn test_field_validator_missing_context_field() {
    // Validator depends on context field that doesn't exist
    let validator = FnFieldValidator::new("end_date", |value, ctx| {
        // Try to access non-existent field
        match ctx.get_field("start_date") {
            Some(start) => {
                if let (Value::Int(end), Value::Int(start_val)) = (value, start) {
                    if end <= start_val {
                        return Err(field_error("end_date", "Must be after start_date"));
                    }
                }
                Ok(value.clone())
            }
            None => {
                // Handle missing context gracefully - pass through
                Ok(value.clone())
            }
        }
    });

    let ctx = ValidatorContext::new(); // No start_date set
    let result = FieldValidator::validate(&validator, &Value::Int(100), &ctx);

    // Should not panic, should pass through
    assert!(result.is_ok());
}

// ============================================================================
// Edge Cases - Null, Empty, Special Values
// ============================================================================

#[test]
fn test_field_validator_null_input_before() {
    let validator = FnFieldValidator::new("optional", |value, _ctx| {
        if let Value::Null = value {
            Ok(Value::Null) // Pass through null
        } else if let Value::String(s) = value {
            Ok(Value::String(s.trim().to_string()))
        } else {
            Err(field_error("optional", "Expected string or null"))
        }
    })
    .mode(ValidatorMode::Before);

    let ctx = ValidatorContext::new();

    // Null should pass through
    let result = FieldValidator::validate(&validator, &Value::Null, &ctx);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Value::Null);
}

#[test]
fn test_field_validator_null_input_after() {
    let validator = FnFieldValidator::new("field", |value, _ctx| {
        match value {
            Value::Null => Ok(Value::String("default".to_string())), // Transform null to default
            other => Ok(other.clone()),
        }
    })
    .mode(ValidatorMode::After);

    let ctx = ValidatorContext::new();

    let result = FieldValidator::validate(&validator, &Value::Null, &ctx);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Value::String("default".to_string()));
}

#[test]
fn test_field_validator_empty_string() {
    let validator = FnFieldValidator::new("name", |value, _ctx| {
        if let Value::String(s) = value {
            if s.is_empty() {
                return Err(field_error("name", "Name cannot be empty"));
            }
            Ok(value.clone())
        } else {
            Ok(value.clone())
        }
    });

    let ctx = ValidatorContext::new();

    let result = FieldValidator::validate(&validator, &Value::String("".to_string()), &ctx);
    assert!(result.is_err());
}

#[test]
fn test_field_validator_whitespace_only_string() {
    let validator = FnFieldValidator::new("name", |value, _ctx| {
        if let Value::String(s) = value {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                return Err(field_error("name", "Name cannot be whitespace only"));
            }
            Ok(Value::String(trimmed.to_string()))
        } else {
            Ok(value.clone())
        }
    })
    .mode(ValidatorMode::Before);

    let ctx = ValidatorContext::new();

    // Whitespace only should fail
    let result =
        FieldValidator::validate(&validator, &Value::String("   \t\n  ".to_string()), &ctx);
    assert!(result.is_err());
}

#[test]
fn test_field_validator_unicode_string() {
    let validator = FnFieldValidator::new("message", |value, _ctx| {
        if let Value::String(s) = value {
            // Validate unicode length
            if s.chars().count() > 10 {
                return Err(field_error("message", "Message too long (max 10 chars)"));
            }
            Ok(value.clone())
        } else {
            Ok(value.clone())
        }
    });

    let ctx = ValidatorContext::new();

    // Short unicode string should pass
    let result = FieldValidator::validate(&validator, &Value::String("你好世界".to_string()), &ctx);
    assert!(result.is_ok());

    // Long unicode string should fail
    let result = FieldValidator::validate(
        &validator,
        &Value::String("这是一个非常长的中文字符串测试".to_string()),
        &ctx,
    );
    assert!(result.is_err());
}

#[test]
fn test_field_validator_numeric_edge_values() {
    let validator = FnFieldValidator::new("amount", |value, _ctx| {
        if let Value::Int(n) = value {
            if *n == i64::MIN {
                return Err(field_error("amount", "Value too small"));
            }
            if *n == i64::MAX {
                return Err(field_error("amount", "Value too large"));
            }
            Ok(value.clone())
        } else {
            Ok(value.clone())
        }
    });

    let ctx = ValidatorContext::new();

    // Normal values pass
    assert!(FieldValidator::validate(&validator, &Value::Int(0), &ctx).is_ok());
    assert!(FieldValidator::validate(&validator, &Value::Int(-1000), &ctx).is_ok());
    assert!(FieldValidator::validate(&validator, &Value::Int(1000), &ctx).is_ok());

    // Edge values fail
    assert!(FieldValidator::validate(&validator, &Value::Int(i64::MIN), &ctx).is_err());
    assert!(FieldValidator::validate(&validator, &Value::Int(i64::MAX), &ctx).is_err());
}

#[test]
fn test_field_validator_float_special_values() {
    let validator = FnFieldValidator::new("rate", |value, _ctx| {
        if let Value::Float(n) = value {
            if n.is_nan() {
                return Err(field_error("rate", "NaN not allowed"));
            }
            if n.is_infinite() {
                return Err(field_error("rate", "Infinite value not allowed"));
            }
            Ok(value.clone())
        } else {
            Ok(value.clone())
        }
    });

    let ctx = ValidatorContext::new();

    // Normal floats pass
    assert!(FieldValidator::validate(&validator, &Value::Float(0.0), &ctx).is_ok());
    assert!(FieldValidator::validate(&validator, &Value::Float(-0.0), &ctx).is_ok());
    assert!(FieldValidator::validate(&validator, &Value::Float(3.14159), &ctx).is_ok());

    // Special values fail
    assert!(FieldValidator::validate(&validator, &Value::Float(f64::NAN), &ctx).is_err());
    assert!(FieldValidator::validate(&validator, &Value::Float(f64::INFINITY), &ctx).is_err());
    assert!(FieldValidator::validate(&validator, &Value::Float(f64::NEG_INFINITY), &ctx).is_err());
}

#[test]
fn test_field_validator_empty_list() {
    let validator = FnFieldValidator::new("items", |value, _ctx| {
        if let Value::List(items) = value {
            if items.is_empty() {
                return Err(field_error("items", "List cannot be empty"));
            }
            Ok(value.clone())
        } else {
            Ok(value.clone())
        }
    });

    let ctx = ValidatorContext::new();

    // Empty list should fail
    let result = FieldValidator::validate(&validator, &Value::List(vec![]), &ctx);
    assert!(result.is_err());

    // Non-empty list should pass
    let result = FieldValidator::validate(&validator, &Value::List(vec![Value::Int(1)]), &ctx);
    assert!(result.is_ok());
}

#[test]
fn test_field_validator_list_with_non_string_items() {
    // Validator expecting list of strings
    let validator = FnFieldValidator::new("tags", |value, _ctx| {
        if let Value::List(items) = value {
            for (i, item) in items.iter().enumerate() {
                if !matches!(item, Value::String(_)) {
                    return Err(field_error(
                        "tags",
                        format!("Item at index {} must be a string", i),
                    ));
                }
            }
            Ok(value.clone())
        } else {
            Err(field_error("tags", "Expected list"))
        }
    });

    let ctx = ValidatorContext::new();

    // List with non-string items should fail
    let mixed_list = Value::List(vec![
        Value::String("tag1".to_string()),
        Value::Int(123), // wrong type
        Value::String("tag3".to_string()),
    ]);
    let result = FieldValidator::validate(&validator, &mixed_list, &ctx);
    assert!(result.is_err());
}
