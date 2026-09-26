//! Core validation engine
//!
//! This module implements the main validation logic for all type descriptors.

use crate::constraints::{
    FieldDescriptor, ListConstraints, NumericConstraints, StringConstraints, StringFormat,
};
use crate::errors::{
    ErrorType, ValidationContext, ValidationError, ValidationErrors, ValidationResult,
};
use crate::formats;
use crate::types::{TypeDescriptor, Value};
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

// ============================================================================
// Pre-compiled Pattern Cache (R1)
// ============================================================================

/// Thread-safe cache for compiled regex patterns.
/// Avoids recompiling the same regex on every validation call.
static PATTERN_CACHE: Lazy<Mutex<HashMap<String, Result<Regex, String>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Get or compile a regex pattern from the cache.
fn get_or_compile_pattern(pattern: &str) -> Result<Regex, String> {
    let mut cache = PATTERN_CACHE.lock().unwrap();
    if let Some(cached) = cache.get(pattern) {
        return cached.clone();
    }
    let result = Regex::new(pattern).map_err(|e| e.to_string());
    cache.insert(pattern.to_string(), result.clone());
    result
}

// ============================================================================
// Public API
// ============================================================================

/// Validate a value against a type descriptor
///
/// This is the main entry point for validation. It validates a value against
/// a type descriptor and returns Ok(()) if validation succeeds, or Err with
/// a collection of validation errors if it fails.
///
/// # Example
///
/// ```
/// use schema::{TypeDescriptor, Value, validate};
///
/// let email_type = TypeDescriptor::Email;
/// let value = Value::String("user@example.com".to_string());
///
/// let result = validate(&value, &email_type);
/// assert!(result.is_ok());
/// ```
pub fn validate(value: &Value, type_desc: &TypeDescriptor) -> ValidationResult<()> {
    let mut ctx = ValidationContext::new();
    let mut errors = ValidationErrors::new();

    validate_value(value, type_desc, &mut ctx, &mut errors);

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Validate a value against a type descriptor with custom context
///
/// This function allows you to provide a custom validation context for
/// tracking the location in nested structures.
pub fn validate_with_context(
    value: &Value,
    type_desc: &TypeDescriptor,
    ctx: &mut ValidationContext,
) -> ValidationResult<()> {
    let mut errors = ValidationErrors::new();
    validate_value(value, type_desc, ctx, &mut errors);
    errors.into_result()
}

// ============================================================================
// Core Validation Function
// ============================================================================

/// Validate a value against a type descriptor
///
/// This is the core validation function that recursively validates values.
/// It accumulates errors rather than returning on first error.
pub fn validate_value(
    value: &Value,
    type_desc: &TypeDescriptor,
    ctx: &mut ValidationContext,
    errors: &mut ValidationErrors,
) {
    match type_desc {
        TypeDescriptor::String(constraints) => {
            validate_string(value, constraints, ctx, errors);
        }
        TypeDescriptor::Int64(constraints) => {
            validate_int64(value, constraints, ctx, errors);
        }
        TypeDescriptor::Float64(constraints) => {
            validate_float64(value, constraints, ctx, errors);
        }
        TypeDescriptor::Bool => {
            validate_bool(value, ctx, errors);
        }
        TypeDescriptor::Null => {
            validate_null(value, ctx, errors);
        }
        TypeDescriptor::Bytes => {
            validate_bytes(value, ctx, errors);
        }
        TypeDescriptor::List { items, constraints } => {
            validate_list(value, items, constraints, ctx, errors);
        }
        TypeDescriptor::Tuple { items } => {
            validate_tuple(value, items, ctx, errors);
        }
        TypeDescriptor::Set { items } => {
            validate_set(value, items, ctx, errors);
        }
        TypeDescriptor::Object { fields, additional } => {
            validate_object(value, fields, additional.as_deref(), ctx, errors);
        }
        TypeDescriptor::Optional(inner) => {
            validate_optional(value, inner, ctx, errors);
        }
        TypeDescriptor::Union { variants, nullable } => {
            validate_union(value, variants, *nullable, ctx, errors);
        }
        TypeDescriptor::Enum { values } => {
            validate_enum(value, values, ctx, errors);
        }
        TypeDescriptor::Literal { values } => {
            validate_literal(value, values, ctx, errors);
        }
        TypeDescriptor::Email => {
            validate_email(value, ctx, errors);
        }
        TypeDescriptor::Url => {
            validate_url(value, ctx, errors);
        }
        TypeDescriptor::Uuid => {
            validate_uuid(value, ctx, errors);
        }
        TypeDescriptor::DateTime => {
            validate_datetime(value, ctx, errors);
        }
        TypeDescriptor::Date => {
            validate_date(value, ctx, errors);
        }
        TypeDescriptor::Time => {
            validate_time(value, ctx, errors);
        }
        TypeDescriptor::Decimal(constraints) => {
            validate_decimal(value, constraints, ctx, errors);
        }
        #[cfg(feature = "bson")]
        TypeDescriptor::ObjectId => {
            validate_bson_objectid(value, ctx, errors);
        }
        #[cfg(feature = "bson")]
        TypeDescriptor::BsonDateTime => {
            validate_bson_datetime(value, ctx, errors);
        }
        #[cfg(feature = "bson")]
        TypeDescriptor::BsonDecimal128 => {
            validate_bson_decimal128(value, ctx, errors);
        }
        #[cfg(feature = "bson")]
        TypeDescriptor::BsonBinary => {
            validate_bson_binary(value, ctx, errors);
        }
        TypeDescriptor::Any => {
            // No validation
        }
        // New format types (Phase 1)
        TypeDescriptor::Ipv4 => {
            validate_ipv4(value, ctx, errors);
        }
        TypeDescriptor::Ipv6 => {
            validate_ipv6(value, ctx, errors);
        }
        TypeDescriptor::Hostname => {
            validate_hostname(value, ctx, errors);
        }
        TypeDescriptor::Fqdn => {
            validate_fqdn(value, ctx, errors);
        }
        TypeDescriptor::Phone => {
            validate_phone(value, ctx, errors);
        }
        TypeDescriptor::Base64 => {
            validate_base64(value, ctx, errors);
        }
        TypeDescriptor::Slug => {
            validate_slug(value, ctx, errors);
        }
        TypeDescriptor::JsonString => {
            validate_json_string(value, ctx, errors);
        }
        TypeDescriptor::TypeParam(name) => {
            // Unresolved type parameter - should be substituted before validation
            errors.add(ValidationError::type_error(
                ctx.location(),
                ctx.field(),
                format!(
                    "Unresolved type parameter '{}'. Generic types must be instantiated before validation.",
                    name
                ),
            ));
        }
    }
}

// ============================================================================
// String Validation
// ============================================================================

fn validate_string(
    value: &Value,
    constraints: &StringConstraints,
    ctx: &mut ValidationContext,
    errors: &mut ValidationErrors,
) {
    match value {
        Value::String(s) => {
            // Check length constraints (in characters, not bytes)
            // R3: For ASCII strings, byte length == char count (O(1) vs O(n))
            let char_count = if s.is_ascii() {
                s.len()
            } else {
                s.chars().count()
            };

            if let Some(min) = constraints.min_length {
                if char_count < min {
                    errors.add(ValidationError::value_error(
                        ctx.location(),
                        ctx.field(),
                        format!(
                            "String must be at least {} characters (got {})",
                            min, char_count
                        ),
                    ));
                }
            }

            if let Some(max) = constraints.max_length {
                if char_count > max {
                    errors.add(ValidationError::value_error(
                        ctx.location(),
                        ctx.field(),
                        format!(
                            "String must be at most {} characters (got {})",
                            max, char_count
                        ),
                    ));
                }
            }

            // Check pattern (regex) - R1: Use pre-compiled pattern cache
            if let Some(pattern) = &constraints.pattern {
                match get_or_compile_pattern(pattern) {
                    Ok(re) => {
                        if !re.is_match(s) {
                            errors.add(ValidationError::value_error(
                                ctx.location(),
                                ctx.field(),
                                format!("String does not match pattern: {}", pattern),
                            ));
                        }
                    }
                    Err(_) => {
                        errors.add(ValidationError::value_error(
                            ctx.location(),
                            ctx.field(),
                            format!("Invalid regex pattern: {}", pattern),
                        ));
                    }
                }
            }

            // Check format
            if let Some(format) = constraints.format {
                validate_string_format(s, format, ctx, errors);
            }
        }
        _ => {
            errors.add(ValidationError::type_error(
                ctx.location(),
                ctx.field(),
                format!("Expected string, got {}", value.type_name()),
            ));
        }
    }
}

fn validate_string_format(
    s: &str,
    format: StringFormat,
    ctx: &ValidationContext,
    errors: &mut ValidationErrors,
) {
    let is_valid = match format {
        StringFormat::Email => formats::validate_email(s),
        StringFormat::Url => formats::validate_url(s),
        StringFormat::Uuid => formats::validate_uuid(s),
        StringFormat::DateTime => formats::validate_datetime(s),
        StringFormat::Date => formats::validate_date(s),
        StringFormat::Time => formats::validate_time(s),
        // New formats (Phase 1)
        StringFormat::Ipv4 => formats::validate_ipv4(s),
        StringFormat::Ipv6 => formats::validate_ipv6(s),
        StringFormat::Hostname => formats::validate_hostname(s),
        StringFormat::Fqdn => formats::validate_fqdn(s),
        StringFormat::Phone => formats::validate_phone(s),
        StringFormat::Base64 => formats::validate_base64(s),
        StringFormat::Slug => formats::validate_slug(s),
        StringFormat::Json => formats::validate_json(s),
    };

    if !is_valid {
        let format_name = match format {
            StringFormat::Email => "email",
            StringFormat::Url => "URL",
            StringFormat::Uuid => "UUID",
            StringFormat::DateTime => "datetime (ISO 8601)",
            StringFormat::Date => "date (YYYY-MM-DD)",
            StringFormat::Time => "time (HH:MM:SS)",
            // New formats (Phase 1)
            StringFormat::Ipv4 => "IPv4 address",
            StringFormat::Ipv6 => "IPv6 address",
            StringFormat::Hostname => "hostname",
            StringFormat::Fqdn => "FQDN",
            StringFormat::Phone => "E.164 phone number",
            StringFormat::Base64 => "base64",
            StringFormat::Slug => "URL slug",
            StringFormat::Json => "JSON",
        };
        errors.add(ValidationError::new(
            ctx.location(),
            ctx.field(),
            format!("Invalid {} format", format_name),
            ErrorType::FormatError,
        ));
    }
}

// ============================================================================
// Numeric Validation
// ============================================================================

fn validate_int64(
    value: &Value,
    constraints: &NumericConstraints<i64>,
    ctx: &mut ValidationContext,
    errors: &mut ValidationErrors,
) {
    match value {
        Value::Int(n) => {
            validate_numeric_constraints(*n, constraints, ctx, errors);

            // Check multiple_of for integers
            if let Some(multiple) = constraints.multiple_of {
                validate_int64_multiple_of(*n, multiple, ctx, errors);
            }
        }
        _ => {
            errors.add(ValidationError::type_error(
                ctx.location(),
                ctx.field(),
                format!("Expected integer, got {}", value.type_name()),
            ));
        }
    }
}

fn validate_float64(
    value: &Value,
    constraints: &NumericConstraints<f64>,
    ctx: &mut ValidationContext,
    errors: &mut ValidationErrors,
) {
    let num = match value {
        Value::Float(f) => *f,
        Value::Int(i) => *i as f64,
        _ => {
            errors.add(ValidationError::type_error(
                ctx.location(),
                ctx.field(),
                format!("Expected number, got {}", value.type_name()),
            ));
            return;
        }
    };

    validate_numeric_constraints(num, constraints, ctx, errors);

    // Check multiple_of for floats
    if let Some(multiple) = constraints.multiple_of {
        validate_float64_multiple_of(num, multiple, ctx, errors);
    }
}

fn validate_numeric_constraints<T>(
    value: T,
    constraints: &NumericConstraints<T>,
    ctx: &ValidationContext,
    errors: &mut ValidationErrors,
) where
    T: PartialOrd + std::fmt::Display + Copy,
{
    // Minimum (inclusive)
    if let Some(min) = constraints.minimum {
        if value < min {
            errors.add(ValidationError::value_error(
                ctx.location(),
                ctx.field(),
                format!("Value must be >= {} (got {})", min, value),
            ));
        }
    }

    // Maximum (inclusive)
    if let Some(max) = constraints.maximum {
        if value > max {
            errors.add(ValidationError::value_error(
                ctx.location(),
                ctx.field(),
                format!("Value must be <= {} (got {})", max, value),
            ));
        }
    }

    // Exclusive minimum
    if let Some(min) = constraints.exclusive_minimum {
        if value <= min {
            errors.add(ValidationError::value_error(
                ctx.location(),
                ctx.field(),
                format!("Value must be > {} (got {})", min, value),
            ));
        }
    }

    // Exclusive maximum
    if let Some(max) = constraints.exclusive_maximum {
        if value >= max {
            errors.add(ValidationError::value_error(
                ctx.location(),
                ctx.field(),
                format!("Value must be < {} (got {})", max, value),
            ));
        }
    }
}

// Special handling for multiple_of with i64
fn validate_int64_multiple_of(
    n: i64,
    multiple: i64,
    ctx: &ValidationContext,
    errors: &mut ValidationErrors,
) {
    if multiple != 0 && n % multiple != 0 {
        errors.add(ValidationError::value_error(
            ctx.location(),
            ctx.field(),
            format!("Value must be a multiple of {}", multiple),
        ));
    }
}

// Special handling for multiple_of with f64 (use epsilon)
fn validate_float64_multiple_of(
    num: f64,
    multiple: f64,
    ctx: &ValidationContext,
    errors: &mut ValidationErrors,
) {
    if multiple != 0.0 {
        let remainder = (num % multiple).abs();
        let tolerance = multiple * 0.0001; // 0.01% tolerance
        if remainder > tolerance && (multiple - remainder).abs() > tolerance {
            errors.add(ValidationError::value_error(
                ctx.location(),
                ctx.field(),
                format!("Value must be a multiple of {}", multiple),
            ));
        }
    }
}

// ============================================================================
// Other Primitive Types
// ============================================================================

fn validate_bool(value: &Value, ctx: &ValidationContext, errors: &mut ValidationErrors) {
    if !matches!(value, Value::Bool(_)) {
        errors.add(ValidationError::type_error(
            ctx.location(),
            ctx.field(),
            format!("Expected boolean, got {}", value.type_name()),
        ));
    }
}

fn validate_null(value: &Value, ctx: &ValidationContext, errors: &mut ValidationErrors) {
    if !value.is_null() {
        errors.add(ValidationError::type_error(
            ctx.location(),
            ctx.field(),
            format!("Expected null, got {}", value.type_name()),
        ));
    }
}

fn validate_bytes(value: &Value, ctx: &ValidationContext, errors: &mut ValidationErrors) {
    if !matches!(value, Value::Bytes(_)) {
        errors.add(ValidationError::type_error(
            ctx.location(),
            ctx.field(),
            format!("Expected bytes, got {}", value.type_name()),
        ));
    }
}

// ============================================================================
// Collection Types
// ============================================================================

fn validate_list(
    value: &Value,
    item_type: &TypeDescriptor,
    constraints: &ListConstraints,
    ctx: &mut ValidationContext,
    errors: &mut ValidationErrors,
) {
    match value {
        Value::List(items) => {
            // Check length constraints
            if let Some(min) = constraints.min_items {
                if items.len() < min {
                    errors.add(ValidationError::value_error(
                        ctx.location(),
                        ctx.field(),
                        format!(
                            "List must have at least {} items (got {})",
                            min,
                            items.len()
                        ),
                    ));
                }
            }

            if let Some(max) = constraints.max_items {
                if items.len() > max {
                    errors.add(ValidationError::value_error(
                        ctx.location(),
                        ctx.field(),
                        format!("List must have at most {} items (got {})", max, items.len()),
                    ));
                }
            }

            // Check uniqueness if required
            if constraints.unique_items {
                let mut seen = HashSet::new();
                for (i, item) in items.iter().enumerate() {
                    let key = format!("{:?}", item);
                    if !seen.insert(key) {
                        errors.add(ValidationError::value_error(
                            ctx.location(),
                            ctx.field(),
                            format!("List contains duplicate value at index {}", i),
                        ));
                    }
                }
            }

            // Validate each item
            for (i, item) in items.iter().enumerate() {
                ctx.push(&format!("[{}]", i));
                validate_value(item, item_type, ctx, errors);
                ctx.pop();
            }
        }
        _ => {
            errors.add(ValidationError::type_error(
                ctx.location(),
                ctx.field(),
                format!("Expected array, got {}", value.type_name()),
            ));
        }
    }
}

fn validate_tuple(
    value: &Value,
    item_types: &[TypeDescriptor],
    ctx: &mut ValidationContext,
    errors: &mut ValidationErrors,
) {
    match value {
        Value::List(items) => {
            // Check length matches
            if items.len() != item_types.len() {
                errors.add(ValidationError::value_error(
                    ctx.location(),
                    ctx.field(),
                    format!(
                        "Tuple must have exactly {} items (got {})",
                        item_types.len(),
                        items.len()
                    ),
                ));
                return;
            }

            // Validate each item with its corresponding type
            for (i, (item, item_type)) in items.iter().zip(item_types.iter()).enumerate() {
                ctx.push(&format!("[{}]", i));
                validate_value(item, item_type, ctx, errors);
                ctx.pop();
            }
        }
        _ => {
            errors.add(ValidationError::type_error(
                ctx.location(),
                ctx.field(),
                format!("Expected tuple (array), got {}", value.type_name()),
            ));
        }
    }
}

fn validate_set(
    value: &Value,
    item_type: &TypeDescriptor,
    ctx: &mut ValidationContext,
    errors: &mut ValidationErrors,
) {
    match value {
        Value::List(items) => {
            // Check for duplicates
            let mut seen = HashSet::new();
            for (i, item) in items.iter().enumerate() {
                let key = format!("{:?}", item);
                if !seen.insert(key) {
                    errors.add(ValidationError::value_error(
                        ctx.location(),
                        ctx.field(),
                        format!("Set contains duplicate value at index {}", i),
                    ));
                }

                // Validate item type
                ctx.push(&format!("[{}]", i));
                validate_value(item, item_type, ctx, errors);
                ctx.pop();
            }
        }
        _ => {
            errors.add(ValidationError::type_error(
                ctx.location(),
                ctx.field(),
                format!(
                    "Expected set (array with unique items), got {}",
                    value.type_name()
                ),
            ));
        }
    }
}

fn validate_object(
    value: &Value,
    fields: &[FieldDescriptor],
    additional: Option<&TypeDescriptor>,
    ctx: &mut ValidationContext,
    errors: &mut ValidationErrors,
) {
    match value {
        Value::Object(pairs) => {
            let obj_map: HashMap<&str, &Value> =
                pairs.iter().map(|(k, v)| (k.as_str(), v)).collect();

            // Validate required and known fields
            for field_desc in fields {
                match obj_map.get(field_desc.name.as_str()) {
                    Some(field_value) => {
                        ctx.push(&field_desc.name);
                        validate_value(field_value, &field_desc.type_desc, ctx, errors);
                        ctx.pop();
                    }
                    None if field_desc.required => {
                        if field_desc.default.is_none() {
                            ctx.push(&field_desc.name);
                            errors.add(ValidationError::missing_error(ctx.location(), ctx.field()));
                            ctx.pop();
                        }
                    }
                    None => {
                        // Optional field missing, no error
                    }
                }
            }

            // Validate additional properties if specified
            if let Some(additional_type) = additional {
                let known_fields: HashSet<&str> = fields.iter().map(|f| f.name.as_str()).collect();

                for (key, val) in pairs {
                    if !known_fields.contains(key.as_str()) {
                        ctx.push(key);
                        validate_value(val, additional_type, ctx, errors);
                        ctx.pop();
                    }
                }
            }
        }
        _ => {
            errors.add(ValidationError::type_error(
                ctx.location(),
                ctx.field(),
                format!("Expected object, got {}", value.type_name()),
            ));
        }
    }
}

// ============================================================================
// Special Types
// ============================================================================

fn validate_optional(
    value: &Value,
    inner: &TypeDescriptor,
    ctx: &mut ValidationContext,
    errors: &mut ValidationErrors,
) {
    if !value.is_null() {
        validate_value(value, inner, ctx, errors);
    }
}

fn validate_union(
    value: &Value,
    variants: &[TypeDescriptor],
    nullable: bool,
    ctx: &mut ValidationContext,
    errors: &mut ValidationErrors,
) {
    // Check null
    if value.is_null() {
        if nullable {
            return; // null is allowed
        } else {
            errors.add(ValidationError::type_error(
                ctx.location(),
                ctx.field(),
                "Value cannot be null".to_string(),
            ));
            return;
        }
    }

    // Try each variant
    for variant in variants {
        let mut temp_errors = ValidationErrors::new();
        let mut temp_ctx = ctx.clone();
        validate_value(value, variant, &mut temp_ctx, &mut temp_errors);
        if temp_errors.is_empty() {
            return; // Match successful
        }
    }

    // No variant matched
    let variant_types: Vec<String> = variants.iter().map(|v| v.type_name().to_string()).collect();
    errors.add(ValidationError::type_error(
        ctx.location(),
        ctx.field(),
        format!(
            "Value does not match any of: [{}]",
            variant_types.join(", ")
        ),
    ));
}

fn validate_enum(
    value: &Value,
    allowed_values: &[Value],
    ctx: &ValidationContext,
    errors: &mut ValidationErrors,
) {
    if !allowed_values.contains(value) {
        let formatted_values: Vec<String> = allowed_values
            .iter()
            .map(|v| match v {
                Value::String(s) => format!("\"{}\"", s),
                Value::Int(i) => i.to_string(),
                Value::Float(f) => f.to_string(),
                Value::Bool(b) => b.to_string(),
                _ => format!("{:?}", v),
            })
            .collect();

        errors.add(ValidationError::value_error(
            ctx.location(),
            ctx.field(),
            format!("Value must be one of: [{}]", formatted_values.join(", ")),
        ));
    }
}

fn validate_literal(
    value: &Value,
    allowed_values: &[Value],
    ctx: &ValidationContext,
    errors: &mut ValidationErrors,
) {
    // Literal and Enum validation logic is the same
    validate_enum(value, allowed_values, ctx, errors);
}

// ============================================================================
// Format Types
// ============================================================================

fn validate_email(value: &Value, ctx: &ValidationContext, errors: &mut ValidationErrors) {
    match value {
        Value::String(s) => {
            if !formats::validate_email(s) {
                errors.add(ValidationError::new(
                    ctx.location(),
                    ctx.field(),
                    "Invalid email format".to_string(),
                    ErrorType::FormatError,
                ));
            }
        }
        _ => {
            errors.add(ValidationError::type_error(
                ctx.location(),
                ctx.field(),
                format!("Expected email string, got {}", value.type_name()),
            ));
        }
    }
}

fn validate_url(value: &Value, ctx: &ValidationContext, errors: &mut ValidationErrors) {
    match value {
        Value::String(s) => {
            if !formats::validate_url(s) {
                errors.add(ValidationError::new(
                    ctx.location(),
                    ctx.field(),
                    "Invalid URL format".to_string(),
                    ErrorType::FormatError,
                ));
            }
        }
        _ => {
            errors.add(ValidationError::type_error(
                ctx.location(),
                ctx.field(),
                format!("Expected URL string, got {}", value.type_name()),
            ));
        }
    }
}

fn validate_uuid(value: &Value, ctx: &ValidationContext, errors: &mut ValidationErrors) {
    match value {
        Value::String(s) => {
            if !formats::validate_uuid(s) {
                errors.add(ValidationError::new(
                    ctx.location(),
                    ctx.field(),
                    "Invalid UUID format".to_string(),
                    ErrorType::FormatError,
                ));
            }
        }
        _ => {
            errors.add(ValidationError::type_error(
                ctx.location(),
                ctx.field(),
                format!("Expected UUID string, got {}", value.type_name()),
            ));
        }
    }
}

fn validate_datetime(value: &Value, ctx: &ValidationContext, errors: &mut ValidationErrors) {
    match value {
        Value::String(s) => {
            if !formats::validate_datetime(s) {
                errors.add(ValidationError::new(
                    ctx.location(),
                    ctx.field(),
                    "Invalid datetime format (expected ISO 8601)".to_string(),
                    ErrorType::FormatError,
                ));
            }
        }
        _ => {
            errors.add(ValidationError::type_error(
                ctx.location(),
                ctx.field(),
                format!("Expected datetime string, got {}", value.type_name()),
            ));
        }
    }
}

fn validate_date(value: &Value, ctx: &ValidationContext, errors: &mut ValidationErrors) {
    match value {
        Value::String(s) => {
            if !formats::validate_date(s) {
                errors.add(ValidationError::new(
                    ctx.location(),
                    ctx.field(),
                    "Invalid date format (expected YYYY-MM-DD)".to_string(),
                    ErrorType::FormatError,
                ));
            }
        }
        _ => {
            errors.add(ValidationError::type_error(
                ctx.location(),
                ctx.field(),
                format!("Expected date string, got {}", value.type_name()),
            ));
        }
    }
}

fn validate_time(value: &Value, ctx: &ValidationContext, errors: &mut ValidationErrors) {
    match value {
        Value::String(s) => {
            if !formats::validate_time(s) {
                errors.add(ValidationError::new(
                    ctx.location(),
                    ctx.field(),
                    "Invalid time format (expected HH:MM:SS)".to_string(),
                    ErrorType::FormatError,
                ));
            }
        }
        _ => {
            errors.add(ValidationError::type_error(
                ctx.location(),
                ctx.field(),
                format!("Expected time string, got {}", value.type_name()),
            ));
        }
    }
}

fn validate_decimal(
    value: &Value,
    constraints: &NumericConstraints<f64>,
    ctx: &mut ValidationContext,
    errors: &mut ValidationErrors,
) {
    // Decimal is validated as a float with constraints
    validate_float64(value, constraints, ctx, errors);
}

// ============================================================================
// New Format Validators (Phase 1)
// ============================================================================

fn validate_ipv4(value: &Value, ctx: &ValidationContext, errors: &mut ValidationErrors) {
    match value {
        Value::String(s) => {
            if !formats::validate_ipv4(s) {
                errors.add(ValidationError::new(
                    ctx.location(),
                    ctx.field(),
                    "Invalid IPv4 address format".to_string(),
                    ErrorType::FormatError,
                ));
            }
        }
        _ => {
            errors.add(ValidationError::type_error(
                ctx.location(),
                ctx.field(),
                format!("Expected IPv4 string, got {}", value.type_name()),
            ));
        }
    }
}

fn validate_ipv6(value: &Value, ctx: &ValidationContext, errors: &mut ValidationErrors) {
    match value {
        Value::String(s) => {
            if !formats::validate_ipv6(s) {
                errors.add(ValidationError::new(
                    ctx.location(),
                    ctx.field(),
                    "Invalid IPv6 address format".to_string(),
                    ErrorType::FormatError,
                ));
            }
        }
        _ => {
            errors.add(ValidationError::type_error(
                ctx.location(),
                ctx.field(),
                format!("Expected IPv6 string, got {}", value.type_name()),
            ));
        }
    }
}

fn validate_hostname(value: &Value, ctx: &ValidationContext, errors: &mut ValidationErrors) {
    match value {
        Value::String(s) => {
            if !formats::validate_hostname(s) {
                errors.add(ValidationError::new(
                    ctx.location(),
                    ctx.field(),
                    "Invalid hostname format".to_string(),
                    ErrorType::FormatError,
                ));
            }
        }
        _ => {
            errors.add(ValidationError::type_error(
                ctx.location(),
                ctx.field(),
                format!("Expected hostname string, got {}", value.type_name()),
            ));
        }
    }
}

fn validate_fqdn(value: &Value, ctx: &ValidationContext, errors: &mut ValidationErrors) {
    match value {
        Value::String(s) => {
            if !formats::validate_fqdn(s) {
                errors.add(ValidationError::new(
                    ctx.location(),
                    ctx.field(),
                    "Invalid FQDN format".to_string(),
                    ErrorType::FormatError,
                ));
            }
        }
        _ => {
            errors.add(ValidationError::type_error(
                ctx.location(),
                ctx.field(),
                format!("Expected FQDN string, got {}", value.type_name()),
            ));
        }
    }
}

fn validate_phone(value: &Value, ctx: &ValidationContext, errors: &mut ValidationErrors) {
    match value {
        Value::String(s) => {
            if !formats::validate_phone(s) {
                errors.add(ValidationError::new(
                    ctx.location(),
                    ctx.field(),
                    "Invalid E.164 phone number format".to_string(),
                    ErrorType::FormatError,
                ));
            }
        }
        _ => {
            errors.add(ValidationError::type_error(
                ctx.location(),
                ctx.field(),
                format!("Expected phone number string, got {}", value.type_name()),
            ));
        }
    }
}

fn validate_base64(value: &Value, ctx: &ValidationContext, errors: &mut ValidationErrors) {
    match value {
        Value::String(s) => {
            if !formats::validate_base64(s) {
                errors.add(ValidationError::new(
                    ctx.location(),
                    ctx.field(),
                    "Invalid base64 format".to_string(),
                    ErrorType::FormatError,
                ));
            }
        }
        _ => {
            errors.add(ValidationError::type_error(
                ctx.location(),
                ctx.field(),
                format!("Expected base64 string, got {}", value.type_name()),
            ));
        }
    }
}

fn validate_slug(value: &Value, ctx: &ValidationContext, errors: &mut ValidationErrors) {
    match value {
        Value::String(s) => {
            if !formats::validate_slug(s) {
                errors.add(ValidationError::new(
                    ctx.location(),
                    ctx.field(),
                    "Invalid URL slug format".to_string(),
                    ErrorType::FormatError,
                ));
            }
        }
        _ => {
            errors.add(ValidationError::type_error(
                ctx.location(),
                ctx.field(),
                format!("Expected slug string, got {}", value.type_name()),
            ));
        }
    }
}

fn validate_json_string(value: &Value, ctx: &ValidationContext, errors: &mut ValidationErrors) {
    match value {
        Value::String(s) => {
            if !formats::validate_json(s) {
                errors.add(ValidationError::new(
                    ctx.location(),
                    ctx.field(),
                    "Invalid JSON format".to_string(),
                    ErrorType::FormatError,
                ));
            }
        }
        _ => {
            errors.add(ValidationError::type_error(
                ctx.location(),
                ctx.field(),
                format!("Expected JSON string, got {}", value.type_name()),
            ));
        }
    }
}

// ============================================================================
// BSON Types (feature-gated)
// ============================================================================

#[cfg(feature = "bson")]
fn validate_bson_objectid(value: &Value, ctx: &ValidationContext, errors: &mut ValidationErrors) {
    // For now, accept any bytes of length 12 or a 24-character hex string
    match value {
        Value::Bytes(b) if b.len() == 12 => {
            // Valid ObjectId bytes
        }
        Value::String(s) if s.len() == 24 && s.chars().all(|c| c.is_ascii_hexdigit()) => {
            // Valid ObjectId hex string
        }
        _ => {
            errors.add(ValidationError::type_error(
                ctx.location(),
                ctx.field(),
                format!(
                    "Expected ObjectId (12 bytes or 24-char hex), got {}",
                    value.type_name()
                ),
            ));
        }
    }
}

#[cfg(feature = "bson")]
fn validate_bson_datetime(value: &Value, ctx: &ValidationContext, errors: &mut ValidationErrors) {
    // Accept int (timestamp) or datetime string
    match value {
        Value::Int(_) => {
            // Valid timestamp
        }
        Value::String(s) => {
            if !formats::validate_datetime(s) {
                errors.add(ValidationError::type_error(
                    ctx.location(),
                    ctx.field(),
                    "Invalid datetime format".to_string(),
                ));
            }
        }
        _ => {
            errors.add(ValidationError::type_error(
                ctx.location(),
                ctx.field(),
                format!(
                    "Expected datetime (int or string), got {}",
                    value.type_name()
                ),
            ));
        }
    }
}

#[cfg(feature = "bson")]
fn validate_bson_decimal128(value: &Value, ctx: &ValidationContext, errors: &mut ValidationErrors) {
    // Accept float, int, or string representation
    match value {
        Value::Float(_) | Value::Int(_) | Value::String(_) => {
            // Valid decimal representation
        }
        _ => {
            errors.add(ValidationError::type_error(
                ctx.location(),
                ctx.field(),
                format!(
                    "Expected decimal (number or string), got {}",
                    value.type_name()
                ),
            ));
        }
    }
}

#[cfg(feature = "bson")]
fn validate_bson_binary(value: &Value, ctx: &ValidationContext, errors: &mut ValidationErrors) {
    if !matches!(value, Value::Bytes(_)) {
        errors.add(ValidationError::type_error(
            ctx.location(),
            ctx.field(),
            format!("Expected binary data, got {}", value.type_name()),
        ));
    }
}
