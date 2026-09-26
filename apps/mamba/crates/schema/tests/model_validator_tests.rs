//! Model Validator Tests
//!
//! Tests for model-level validation (cross-field validation),
//! similar to Pydantic's @model_validator.

use schema::custom_validators::*;
use schema::errors::*;
use schema::types::*;

// ============================================================================
// Model Validator Modes
// ============================================================================

#[test]
fn test_model_validator_before_mode() {
    // Before mode: runs before field validation
    let validator = FnModelValidator::new(|value, _ctx| {
        // Transform: add a computed field before validation
        if let Value::Object(mut fields) = value.clone() {
            // Add a 'processed' flag
            fields.push(("_processed".to_string(), Value::Bool(true)));
            Ok(Value::Object(fields))
        } else {
            Ok(value.clone())
        }
    })
    .mode(ValidatorMode::Before);

    let ctx = ValidatorContext::new();
    let input = Value::Object(vec![(
        "name".to_string(),
        Value::String("Alice".to_string()),
    )]);

    let result = ModelValidator::validate(&validator, &input, &ctx).unwrap();
    let Value::Object(fields) = result else {
        panic!("Expected object");
    };

    assert!(fields.iter().any(|(k, _)| k == "_processed"));
}

#[test]
fn test_model_validator_after_mode() {
    // After mode: runs after field validation
    let validator = FnModelValidator::new(|value, _ctx| {
        if let Value::Object(fields) = value {
            // Validate that name is not empty after field validation
            for (key, val) in fields {
                if key == "name" {
                    if let Value::String(s) = val {
                        if s.is_empty() {
                            return Err(field_error("name", "Name cannot be empty"));
                        }
                    }
                }
            }
        }
        Ok(value.clone())
    })
    .mode(ValidatorMode::After);

    let ctx = ValidatorContext::new();

    // Valid case
    let valid = Value::Object(vec![(
        "name".to_string(),
        Value::String("Alice".to_string()),
    )]);
    assert!(ModelValidator::validate(&validator, &valid, &ctx).is_ok());

    // Invalid case
    let invalid = Value::Object(vec![("name".to_string(), Value::String("".to_string()))]);
    assert!(ModelValidator::validate(&validator, &invalid, &ctx).is_err());
}

#[test]
fn test_model_validator_wrap_mode() {
    // Wrap mode: controls the entire validation flow
    let validator = FnModelValidator::new(|value, _ctx| {
        // Wrap mode can choose to transform or reject entirely
        if let Value::Object(fields) = value {
            if fields.is_empty() {
                return Err(field_error("_model", "Model cannot be empty"));
            }
        }
        Ok(value.clone())
    })
    .mode(ValidatorMode::Wrap);

    let ctx = ValidatorContext::new();

    // Non-empty object is valid
    let valid = Value::Object(vec![("x".to_string(), Value::Int(1))]);
    assert!(ModelValidator::validate(&validator, &valid, &ctx).is_ok());

    // Empty object is invalid
    let invalid = Value::Object(vec![]);
    assert!(ModelValidator::validate(&validator, &invalid, &ctx).is_err());
}

// ============================================================================
// Cross-Field Validation
// ============================================================================

#[test]
fn test_cross_field_validation() {
    // Validate that start_date < end_date using context
    let validator = FnModelValidator::new(|value, _ctx| {
        if let Value::Object(fields) = value {
            let mut start: Option<i64> = None;
            let mut end: Option<i64> = None;

            for (key, val) in fields {
                if key == "start_date" {
                    if let Value::Int(v) = val {
                        start = Some(*v);
                    }
                }
                if key == "end_date" {
                    if let Value::Int(v) = val {
                        end = Some(*v);
                    }
                }
            }

            if let (Some(s), Some(e)) = (start, end) {
                if s >= e {
                    return Err(field_error(
                        "end_date",
                        "end_date must be greater than start_date",
                    ));
                }
            }
        }
        Ok(value.clone())
    });

    let ctx = ValidatorContext::new();

    // Valid: start < end
    let valid = Value::Object(vec![
        ("start_date".to_string(), Value::Int(100)),
        ("end_date".to_string(), Value::Int(200)),
    ]);
    assert!(ModelValidator::validate(&validator, &valid, &ctx).is_ok());

    // Invalid: start >= end
    let invalid = Value::Object(vec![
        ("start_date".to_string(), Value::Int(200)),
        ("end_date".to_string(), Value::Int(100)),
    ]);
    assert!(ModelValidator::validate(&validator, &invalid, &ctx).is_err());

    // Invalid: start == end
    let invalid2 = Value::Object(vec![
        ("start_date".to_string(), Value::Int(100)),
        ("end_date".to_string(), Value::Int(100)),
    ]);
    assert!(ModelValidator::validate(&validator, &invalid2, &ctx).is_err());
}

#[test]
fn test_password_confirmation_pattern() {
    // Common pattern: password must match confirm_password
    let validator = FnModelValidator::new(|value, _ctx| {
        if let Value::Object(fields) = value {
            let mut password: Option<&str> = None;
            let mut confirm: Option<&str> = None;

            for (key, val) in fields {
                if key == "password" {
                    if let Value::String(s) = val {
                        password = Some(s.as_str());
                    }
                }
                if key == "confirm_password" {
                    if let Value::String(s) = val {
                        confirm = Some(s.as_str());
                    }
                }
            }

            match (password, confirm) {
                (Some(p), Some(c)) if p != c => {
                    return Err(field_error("confirm_password", "Passwords do not match"));
                }
                (Some(_), None) => {
                    return Err(field_error(
                        "confirm_password",
                        "Password confirmation is required",
                    ));
                }
                _ => {}
            }
        }
        Ok(value.clone())
    });

    let ctx = ValidatorContext::new();

    // Valid: passwords match
    let valid = Value::Object(vec![
        (
            "password".to_string(),
            Value::String("secret123".to_string()),
        ),
        (
            "confirm_password".to_string(),
            Value::String("secret123".to_string()),
        ),
    ]);
    assert!(ModelValidator::validate(&validator, &valid, &ctx).is_ok());

    // Invalid: passwords don't match
    let invalid = Value::Object(vec![
        (
            "password".to_string(),
            Value::String("secret123".to_string()),
        ),
        (
            "confirm_password".to_string(),
            Value::String("different".to_string()),
        ),
    ]);
    assert!(ModelValidator::validate(&validator, &invalid, &ctx).is_err());
}

#[test]
fn test_date_range_validation() {
    // Validate date ranges in booking system
    let validator = FnModelValidator::new(|value, _ctx| {
        if let Value::Object(fields) = value {
            let mut check_in: Option<&str> = None;
            let mut check_out: Option<&str> = None;

            for (key, val) in fields {
                if key == "check_in" {
                    if let Value::String(s) = val {
                        check_in = Some(s.as_str());
                    }
                }
                if key == "check_out" {
                    if let Value::String(s) = val {
                        check_out = Some(s.as_str());
                    }
                }
            }

            if let (Some(ci), Some(co)) = (check_in, check_out) {
                // Simple string comparison works for ISO dates
                if ci >= co {
                    return Err(field_error(
                        "check_out",
                        "Check-out date must be after check-in date",
                    ));
                }
            }
        }
        Ok(value.clone())
    });

    let ctx = ValidatorContext::new();

    // Valid
    let valid = Value::Object(vec![
        (
            "check_in".to_string(),
            Value::String("2024-01-15".to_string()),
        ),
        (
            "check_out".to_string(),
            Value::String("2024-01-20".to_string()),
        ),
    ]);
    assert!(ModelValidator::validate(&validator, &valid, &ctx).is_ok());

    // Invalid
    let invalid = Value::Object(vec![
        (
            "check_in".to_string(),
            Value::String("2024-01-20".to_string()),
        ),
        (
            "check_out".to_string(),
            Value::String("2024-01-15".to_string()),
        ),
    ]);
    assert!(ModelValidator::validate(&validator, &invalid, &ctx).is_err());
}

#[test]
fn test_conditional_required_fields() {
    // If 'is_company' is true, 'company_name' is required
    let validator = FnModelValidator::new(|value, _ctx| {
        if let Value::Object(fields) = value {
            let mut is_company = false;
            let mut has_company_name = false;

            for (key, val) in fields {
                if key == "is_company" {
                    if let Value::Bool(b) = val {
                        is_company = *b;
                    }
                }
                if key == "company_name" {
                    if let Value::String(s) = val {
                        has_company_name = !s.is_empty();
                    }
                }
            }

            if is_company && !has_company_name {
                return Err(field_error(
                    "company_name",
                    "Company name is required when is_company is true",
                ));
            }
        }
        Ok(value.clone())
    });

    let ctx = ValidatorContext::new();

    // Valid: is_company=true with company_name
    let valid = Value::Object(vec![
        ("is_company".to_string(), Value::Bool(true)),
        (
            "company_name".to_string(),
            Value::String("Acme Inc".to_string()),
        ),
    ]);
    assert!(ModelValidator::validate(&validator, &valid, &ctx).is_ok());

    // Valid: is_company=false without company_name
    let valid2 = Value::Object(vec![("is_company".to_string(), Value::Bool(false))]);
    assert!(ModelValidator::validate(&validator, &valid2, &ctx).is_ok());

    // Invalid: is_company=true without company_name
    let invalid = Value::Object(vec![("is_company".to_string(), Value::Bool(true))]);
    assert!(ModelValidator::validate(&validator, &invalid, &ctx).is_err());
}

// ============================================================================
// Validator Collection Integration
// ============================================================================

#[test]
fn test_model_validator_with_context() {
    let validator = FnModelValidator::new(|value, ctx| {
        // Access context metadata
        if ctx.get_metadata("strict_mode") == Some("true") {
            // Apply stricter validation
            if let Value::Object(fields) = value {
                if fields.len() < 2 {
                    return Err(field_error(
                        "_model",
                        "At least 2 fields required in strict mode",
                    ));
                }
            }
        }
        Ok(value.clone())
    });

    // Non-strict mode
    let ctx = ValidatorContext::new();
    let single_field = Value::Object(vec![("x".to_string(), Value::Int(1))]);
    assert!(ModelValidator::validate(&validator, &single_field, &ctx).is_ok());

    // Strict mode
    let mut strict_ctx = ValidatorContext::new();
    strict_ctx.set_metadata("strict_mode", "true");
    assert!(ModelValidator::validate(&validator, &single_field, &strict_ctx).is_err());

    // Strict mode with enough fields
    let multi_field = Value::Object(vec![
        ("x".to_string(), Value::Int(1)),
        ("y".to_string(), Value::Int(2)),
    ]);
    assert!(ModelValidator::validate(&validator, &multi_field, &strict_ctx).is_ok());
}

#[test]
fn test_model_validator_chaining() {
    let mut collection = ValidatorCollection::new();

    // First validator: normalize
    collection.add_model_validator(
        FnModelValidator::new(|value, _ctx| {
            if let Value::Object(mut fields) = value.clone() {
                // Add normalized flag
                fields.push(("_normalized".to_string(), Value::Bool(true)));
                Ok(Value::Object(fields))
            } else {
                Ok(value.clone())
            }
        })
        .mode(ValidatorMode::Before),
    );

    // Second validator: validate
    collection.add_model_validator(
        FnModelValidator::new(|value, _ctx| {
            if let Value::Object(fields) = value {
                // Check that normalized flag exists
                let has_flag = fields.iter().any(|(k, _)| k == "_normalized");
                if !has_flag {
                    return Err(field_error("_model", "Not normalized"));
                }
            }
            Ok(value.clone())
        })
        .mode(ValidatorMode::After),
    );

    let ctx = ValidatorContext::new();
    let input = Value::Object(vec![("data".to_string(), Value::Int(42))]);

    // Run before validators
    let after_before = collection
        .run_model_validators(&input, &ctx, ValidatorMode::Before)
        .unwrap();

    // Run after validators on the result
    let final_result = collection.run_model_validators(&after_before, &ctx, ValidatorMode::After);
    assert!(final_result.is_ok());
}

#[test]
fn test_model_validator_error_handling() {
    let validator = FnModelValidator::new(|_value, _ctx| {
        let mut errors = ValidationErrors::new();
        errors.add(ValidationError::new(
            "body".to_string(),
            "field1".to_string(),
            "Error in field1".to_string(),
            ErrorType::ValueError,
        ));
        errors.add(ValidationError::new(
            "body".to_string(),
            "field2".to_string(),
            "Error in field2".to_string(),
            ErrorType::ValueError,
        ));
        Err(errors)
    });

    let ctx = ValidatorContext::new();
    let input = Value::Object(vec![]);

    let result = ModelValidator::validate(&validator, &input, &ctx);
    assert!(result.is_err());

    let errors = result.unwrap_err();
    assert_eq!(errors.len(), 2);
}

// ============================================================================
// Error Cases - Non-Object Input, Multiple Error Types
// ============================================================================

#[test]
fn test_model_validator_non_object_input() {
    // Model validator receiving non-object should handle gracefully
    let validator = FnModelValidator::new(|value, _ctx| match value {
        Value::Object(_) => Ok(value.clone()),
        _ => Err(field_error("_model", "Expected object, got different type")),
    });

    let ctx = ValidatorContext::new();

    // String instead of object
    let result =
        ModelValidator::validate(&validator, &Value::String("not object".to_string()), &ctx);
    assert!(result.is_err());

    // List instead of object
    let result = ModelValidator::validate(&validator, &Value::List(vec![]), &ctx);
    assert!(result.is_err());

    // Int instead of object
    let result = ModelValidator::validate(&validator, &Value::Int(123), &ctx);
    assert!(result.is_err());

    // Null instead of object
    let result = ModelValidator::validate(&validator, &Value::Null, &ctx);
    assert!(result.is_err());
}

#[test]
fn test_model_validator_multiple_error_types() {
    // Return different error types from model validator
    let validator = FnModelValidator::new(|value, _ctx| {
        let mut errors = ValidationErrors::new();

        if let Value::Object(fields) = value {
            // Check for type error
            for (key, val) in fields {
                if key == "age" {
                    if !matches!(val, Value::Int(_)) {
                        errors.add(ValidationError::type_error(
                            "body".to_string(),
                            "age".to_string(),
                            "Expected integer".to_string(),
                        ));
                    }
                }
                if key == "email" {
                    if let Value::String(s) = val {
                        if !s.contains('@') {
                            errors.add(ValidationError::value_error(
                                "body".to_string(),
                                "email".to_string(),
                                "Invalid email format".to_string(),
                            ));
                        }
                    }
                }
            }

            // Check for missing required field
            if !fields.iter().any(|(k, _)| k == "name") {
                errors.add(ValidationError::missing_error(
                    "body".to_string(),
                    "name".to_string(),
                ));
            }
        }

        errors.into_result().map(|_| value.clone())
    });

    let ctx = ValidatorContext::new();

    // Input with multiple error types
    let input = Value::Object(vec![
        ("age".to_string(), Value::String("not-int".to_string())), // TypeError
        ("email".to_string(), Value::String("invalid".to_string())), // ValueError
                                                                   // name is missing - Missing error
    ]);

    let result = ModelValidator::validate(&validator, &input, &ctx);
    assert!(result.is_err());

    let errors = result.unwrap_err();
    assert!(errors.len() >= 3);

    // Check different error types are present
    let has_type_error = errors
        .errors
        .iter()
        .any(|e| e.error_type == ErrorType::TypeError);
    let has_value_error = errors
        .errors
        .iter()
        .any(|e| e.error_type == ErrorType::ValueError);
    let has_missing_error = errors
        .errors
        .iter()
        .any(|e| e.error_type == ErrorType::Missing);

    assert!(has_type_error);
    assert!(has_value_error);
    assert!(has_missing_error);
}

#[test]
fn test_model_validator_conflicting_validators() {
    // Two model validators that both produce errors
    let mut collection = ValidatorCollection::new();

    collection.add_model_validator(
        FnModelValidator::new(|value, _ctx| {
            if let Value::Object(fields) = value {
                if fields.len() < 2 {
                    return Err(field_error("_model", "Need at least 2 fields"));
                }
            }
            Ok(value.clone())
        })
        .mode(ValidatorMode::After),
    );

    collection.add_model_validator(
        FnModelValidator::new(|value, _ctx| {
            if let Value::Object(fields) = value {
                if fields.len() > 5 {
                    return Err(field_error("_model", "Too many fields (max 5)"));
                }
            }
            Ok(value.clone())
        })
        .mode(ValidatorMode::After),
    );

    let ctx = ValidatorContext::new();

    // Input with only 1 field - fails first validator
    let too_few = Value::Object(vec![("x".to_string(), Value::Int(1))]);
    let result = collection.run_model_validators(&too_few, &ctx, ValidatorMode::After);
    assert!(result.is_err());
}

// ============================================================================
// Edge Cases - Empty, Null, Optional Fields
// ============================================================================

#[test]
fn test_model_validator_empty_object_before_after() {
    let before_validator = FnModelValidator::new(|value, _ctx| {
        // Before mode: add default field if empty
        if let Value::Object(fields) = value {
            if fields.is_empty() {
                return Ok(Value::Object(vec![(
                    "default".to_string(),
                    Value::String("added".to_string()),
                )]));
            }
        }
        Ok(value.clone())
    })
    .mode(ValidatorMode::Before);

    let after_validator = FnModelValidator::new(|value, _ctx| {
        // After mode: validate non-empty
        if let Value::Object(fields) = value {
            if fields.is_empty() {
                return Err(field_error(
                    "_model",
                    "Object cannot be empty after validation",
                ));
            }
        }
        Ok(value.clone())
    })
    .mode(ValidatorMode::After);

    let ctx = ValidatorContext::new();
    let empty = Value::Object(vec![]);

    // Before validator transforms empty to have default
    let result = ModelValidator::validate(&before_validator, &empty, &ctx);
    assert!(result.is_ok());
    if let Value::Object(fields) = result.unwrap() {
        assert!(fields.iter().any(|(k, _)| k == "default"));
    }

    // After validator rejects empty
    let result = ModelValidator::validate(&after_validator, &empty, &ctx);
    assert!(result.is_err());
}

#[test]
fn test_model_validator_missing_field_in_cross_validation() {
    // Cross-field validation when one field is missing
    let validator = FnModelValidator::new(|value, _ctx| {
        if let Value::Object(fields) = value {
            let start = fields.iter().find(|(k, _)| k == "start").map(|(_, v)| v);
            let end = fields.iter().find(|(k, _)| k == "end").map(|(_, v)| v);

            match (start, end) {
                (Some(Value::Int(s)), Some(Value::Int(e))) => {
                    if s >= e {
                        return Err(field_error("end", "end must be > start"));
                    }
                }
                (Some(_), None) => {
                    // start present but end missing - that's OK, just pass through
                }
                (None, Some(_)) => {
                    // end present but start missing - that's OK, just pass through
                }
                (None, None) => {
                    // Both missing - that's OK, just pass through
                }
                _ => {}
            }
        }
        Ok(value.clone())
    });

    let ctx = ValidatorContext::new();

    // Only start present - should pass (no false error)
    let only_start = Value::Object(vec![("start".to_string(), Value::Int(100))]);
    assert!(ModelValidator::validate(&validator, &only_start, &ctx).is_ok());

    // Only end present - should pass
    let only_end = Value::Object(vec![("end".to_string(), Value::Int(200))]);
    assert!(ModelValidator::validate(&validator, &only_end, &ctx).is_ok());

    // Neither present - should pass
    let neither = Value::Object(vec![("other".to_string(), Value::Int(1))]);
    assert!(ModelValidator::validate(&validator, &neither, &ctx).is_ok());
}

#[test]
fn test_model_validator_optional_fields_with_null() {
    // Optional fields that are present but null
    let validator = FnModelValidator::new(|value, _ctx| {
        if let Value::Object(fields) = value {
            let nickname = fields.iter().find(|(k, _)| k == "nickname").map(|(_, v)| v);
            let bio = fields.iter().find(|(k, _)| k == "bio").map(|(_, v)| v);

            // If nickname is present and not null, validate it's not empty
            if let Some(Value::String(s)) = nickname {
                if s.is_empty() {
                    return Err(field_error("nickname", "Nickname cannot be empty string"));
                }
            }
            // Null is OK, missing is OK

            // Same for bio
            if let Some(Value::String(s)) = bio {
                if s.len() > 500 {
                    return Err(field_error("bio", "Bio too long (max 500 chars)"));
                }
            }
        }
        Ok(value.clone())
    });

    let ctx = ValidatorContext::new();

    // Null values should pass
    let with_nulls = Value::Object(vec![
        ("nickname".to_string(), Value::Null),
        ("bio".to_string(), Value::Null),
    ]);
    assert!(ModelValidator::validate(&validator, &with_nulls, &ctx).is_ok());

    // Empty string should fail
    let empty_nickname = Value::Object(vec![(
        "nickname".to_string(),
        Value::String("".to_string()),
    )]);
    assert!(ModelValidator::validate(&validator, &empty_nickname, &ctx).is_err());

    // Long bio should fail
    let long_bio = Value::Object(vec![("bio".to_string(), Value::String("x".repeat(501)))]);
    assert!(ModelValidator::validate(&validator, &long_bio, &ctx).is_err());
}

#[test]
fn test_model_validator_non_iso_date_format() {
    // Test with non-ISO date formats (common edge case)
    let validator = FnModelValidator::new(|value, _ctx| {
        if let Value::Object(fields) = value {
            for (key, val) in fields {
                if key == "date" {
                    if let Value::String(s) = val {
                        // Check if it looks like ISO format (YYYY-MM-DD)
                        let is_iso = s.len() == 10
                            && s.chars().nth(4) == Some('-')
                            && s.chars().nth(7) == Some('-');

                        if !is_iso {
                            return Err(field_error(
                                "date",
                                "Date must be in ISO format (YYYY-MM-DD)",
                            ));
                        }
                    }
                }
            }
        }
        Ok(value.clone())
    });

    let ctx = ValidatorContext::new();

    // ISO format passes
    let iso = Value::Object(vec![(
        "date".to_string(),
        Value::String("2024-01-15".to_string()),
    )]);
    assert!(ModelValidator::validate(&validator, &iso, &ctx).is_ok());

    // US format fails
    let us_format = Value::Object(vec![(
        "date".to_string(),
        Value::String("01/15/2024".to_string()),
    )]);
    assert!(ModelValidator::validate(&validator, &us_format, &ctx).is_err());

    // European format fails
    let eu_format = Value::Object(vec![(
        "date".to_string(),
        Value::String("15.01.2024".to_string()),
    )]);
    assert!(ModelValidator::validate(&validator, &eu_format, &ctx).is_err());
}

#[test]
fn test_model_validator_nested_object_validation() {
    // Validate nested object structure
    let validator = FnModelValidator::new(|value, _ctx| {
        if let Value::Object(fields) = value {
            for (key, val) in fields {
                if key == "address" {
                    match val {
                        Value::Object(addr_fields) => {
                            // Must have city and country
                            let has_city = addr_fields.iter().any(|(k, _)| k == "city");
                            let has_country = addr_fields.iter().any(|(k, _)| k == "country");

                            if !has_city {
                                return Err(field_error("address.city", "City is required"));
                            }
                            if !has_country {
                                return Err(field_error("address.country", "Country is required"));
                            }
                        }
                        Value::Null => {
                            // Null address is OK (optional)
                        }
                        _ => {
                            return Err(field_error(
                                "address",
                                "Address must be an object or null",
                            ));
                        }
                    }
                }
            }
        }
        Ok(value.clone())
    });

    let ctx = ValidatorContext::new();

    // Valid address
    let valid = Value::Object(vec![(
        "address".to_string(),
        Value::Object(vec![
            ("city".to_string(), Value::String("Tokyo".to_string())),
            ("country".to_string(), Value::String("Japan".to_string())),
        ]),
    )]);
    assert!(ModelValidator::validate(&validator, &valid, &ctx).is_ok());

    // Null address is OK
    let null_addr = Value::Object(vec![("address".to_string(), Value::Null)]);
    assert!(ModelValidator::validate(&validator, &null_addr, &ctx).is_ok());

    // Missing city fails
    let no_city = Value::Object(vec![(
        "address".to_string(),
        Value::Object(vec![(
            "country".to_string(),
            Value::String("Japan".to_string()),
        )]),
    )]);
    assert!(ModelValidator::validate(&validator, &no_city, &ctx).is_err());

    // Wrong type for address fails
    let wrong_type = Value::Object(vec![(
        "address".to_string(),
        Value::String("123 Main St".to_string()),
    )]);
    assert!(ModelValidator::validate(&validator, &wrong_type, &ctx).is_err());
}
