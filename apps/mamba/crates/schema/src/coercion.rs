//! Type coercion engine for cclab-shield
//!
//! Provides intelligent type conversion similar to Pydantic's lax mode.
//! Supports automatic conversion between compatible types during validation.
//!
//! # Supported Coercions
//!
//! | From | To | Example |
//! |------|-----|---------|
//! | String | Int | "123" → 123 |
//! | String | Float | "3.14" → 3.14 |
//! | String | Bool | "true"/"1" → true |
//! | Int | Float | 1 → 1.0 |
//! | Float | Int | 1.0 → 1 (integer only) |
//! | Bool | Int | true → 1, false → 0 |
//! | Int | Bool | 0 → false, non-0 → true |
//!
//! # Example
//!
//! ```rust,ignore
//! use schema::coercion::{CoercionMode, coerce_value};
//! use schema::types::Value;
//!
//! let value = Value::String("123".to_string());
//! let coerced = coerce_value(&value, &TypeDescriptor::Int64(Default::default()), CoercionMode::Lax);
//! assert_eq!(coerced, Some(Value::Int(123)));
//! ```

use crate::types::{TypeDescriptor, Value};

// ============================================================================
// Constants
// ============================================================================

/// Maximum depth for recursive coercion to prevent stack overflow
pub const MAX_COERCION_DEPTH: usize = 64;

// ============================================================================
// Coercion Mode
// ============================================================================

/// Coercion mode for type conversion
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CoercionMode {
    /// Strict mode - no automatic type conversion
    #[default]
    Strict,
    /// Lax mode - attempt automatic type conversion
    Lax,
}

// ============================================================================
// Coercion Result
// ============================================================================

/// Result of a coercion attempt
#[derive(Debug, Clone, PartialEq)]
pub enum CoercionResult {
    /// Value was already the correct type
    NoChange,
    /// Value was successfully coerced
    Coerced(Value),
    /// Value could not be coerced
    Failed,
}

impl CoercionResult {
    /// Get the coerced value if successful
    pub fn value(&self) -> Option<&Value> {
        match self {
            Self::Coerced(v) => Some(v),
            _ => None,
        }
    }

    /// Check if coercion was successful (including no change needed)
    pub fn is_ok(&self) -> bool {
        !matches!(self, Self::Failed)
    }
}

// ============================================================================
// Coercion Functions
// ============================================================================

/// Check if a value's type matches the target type descriptor
fn value_matches_type(value: &Value, target: &TypeDescriptor) -> bool {
    match (value, target) {
        (Value::Int(_), TypeDescriptor::Int64(_)) => true,
        (Value::Float(_), TypeDescriptor::Float64(_)) => true,
        (Value::Int(_), TypeDescriptor::Float64(_)) => true, // Int is acceptable for Float
        (Value::Bool(_), TypeDescriptor::Bool) => true,
        (Value::String(_), TypeDescriptor::String(_)) => true,
        (Value::Null, TypeDescriptor::Null) => true,
        (Value::Bytes(_), TypeDescriptor::Bytes) => true,
        (Value::List(_), TypeDescriptor::List { .. }) => true,
        (Value::List(_), TypeDescriptor::Tuple { .. }) => true,
        (Value::List(_), TypeDescriptor::Set { .. }) => true,
        (Value::Object(_), TypeDescriptor::Object { .. }) => true,
        (Value::Null, TypeDescriptor::Optional(_)) => true,
        (_, TypeDescriptor::Optional(inner)) => value_matches_type(value, inner),
        (_, TypeDescriptor::Any) => true,
        // Format types expect strings
        (Value::String(_), TypeDescriptor::Email) => true,
        (Value::String(_), TypeDescriptor::Url) => true,
        (Value::String(_), TypeDescriptor::Uuid) => true,
        (Value::String(_), TypeDescriptor::DateTime) => true,
        (Value::String(_), TypeDescriptor::Date) => true,
        (Value::String(_), TypeDescriptor::Time) => true,
        (Value::String(_), TypeDescriptor::Ipv4) => true,
        (Value::String(_), TypeDescriptor::Ipv6) => true,
        (Value::String(_), TypeDescriptor::Hostname) => true,
        (Value::String(_), TypeDescriptor::Fqdn) => true,
        (Value::String(_), TypeDescriptor::Phone) => true,
        (Value::String(_), TypeDescriptor::Base64) => true,
        (Value::String(_), TypeDescriptor::Slug) => true,
        (Value::String(_), TypeDescriptor::JsonString) => true,
        // Decimal accepts both int and float
        (Value::Int(_), TypeDescriptor::Decimal(_)) => true,
        (Value::Float(_), TypeDescriptor::Decimal(_)) => true,
        // TypeParam should be substituted before coercion
        (_, TypeDescriptor::TypeParam(_)) => false,
        _ => false,
    }
}

/// Attempt to coerce a value to match a type descriptor
///
/// In `Strict` mode: Returns `NoChange` if type already matches, `Failed` if not.
/// In `Lax` mode: Attempts automatic type conversion.
pub fn coerce_value(value: &Value, target: &TypeDescriptor, mode: CoercionMode) -> CoercionResult {
    coerce_value_with_depth(value, target, mode, 0)
}

/// Internal coercion function with depth tracking to prevent stack overflow
fn coerce_value_with_depth(
    value: &Value,
    target: &TypeDescriptor,
    mode: CoercionMode,
    depth: usize,
) -> CoercionResult {
    // Depth limit check to prevent stack overflow
    if depth > MAX_COERCION_DEPTH {
        return CoercionResult::Failed;
    }

    if mode == CoercionMode::Strict {
        // In strict mode, check if value type matches target
        return if value_matches_type(value, target) {
            CoercionResult::NoChange
        } else {
            CoercionResult::Failed
        };
    }

    match target {
        TypeDescriptor::Int64(_) => coerce_to_int(value),
        TypeDescriptor::Float64(_) => coerce_to_float(value),
        TypeDescriptor::Bool => coerce_to_bool(value),
        TypeDescriptor::String(_) => coerce_to_string(value),
        TypeDescriptor::Optional(inner) => {
            if matches!(value, Value::Null) {
                return CoercionResult::NoChange;
            }
            coerce_value_with_depth(value, inner, mode, depth + 1)
        }
        TypeDescriptor::List { items, .. } => coerce_list(value, items, mode, depth + 1),
        _ => CoercionResult::NoChange,
    }
}

/// Coerce a list value, recursively coercing each element
fn coerce_list(
    value: &Value,
    item_type: &TypeDescriptor,
    mode: CoercionMode,
    depth: usize,
) -> CoercionResult {
    let Value::List(items) = value else {
        return CoercionResult::Failed;
    };

    let mut coerced_items = Vec::with_capacity(items.len());
    let mut any_coerced = false;

    for item in items {
        match coerce_value_with_depth(item, item_type, mode, depth) {
            CoercionResult::NoChange => coerced_items.push(item.clone()),
            CoercionResult::Coerced(v) => {
                any_coerced = true;
                coerced_items.push(v);
            }
            CoercionResult::Failed => return CoercionResult::Failed,
        }
    }

    if any_coerced {
        CoercionResult::Coerced(Value::List(coerced_items))
    } else {
        CoercionResult::NoChange
    }
}

/// Coerce value to integer
fn coerce_to_int(value: &Value) -> CoercionResult {
    match value {
        Value::Int(_) => CoercionResult::NoChange,
        Value::Float(f) => {
            // Only coerce if it's an integer value
            if f.fract() == 0.0 && *f >= i64::MIN as f64 && *f <= i64::MAX as f64 {
                CoercionResult::Coerced(Value::Int(*f as i64))
            } else {
                CoercionResult::Failed
            }
        }
        Value::String(s) => {
            // Try parsing as integer
            s.trim()
                .parse::<i64>()
                .map(|i| CoercionResult::Coerced(Value::Int(i)))
                .unwrap_or(CoercionResult::Failed)
        }
        Value::Bool(b) => CoercionResult::Coerced(Value::Int(if *b { 1 } else { 0 })),
        _ => CoercionResult::Failed,
    }
}

/// Coerce value to float
fn coerce_to_float(value: &Value) -> CoercionResult {
    match value {
        Value::Float(_) => CoercionResult::NoChange,
        Value::Int(i) => CoercionResult::Coerced(Value::Float(*i as f64)),
        Value::String(s) => {
            // Try parsing as float
            s.trim()
                .parse::<f64>()
                .map(|f| CoercionResult::Coerced(Value::Float(f)))
                .unwrap_or(CoercionResult::Failed)
        }
        Value::Bool(b) => CoercionResult::Coerced(Value::Float(if *b { 1.0 } else { 0.0 })),
        _ => CoercionResult::Failed,
    }
}

/// Coerce value to boolean
fn coerce_to_bool(value: &Value) -> CoercionResult {
    match value {
        Value::Bool(_) => CoercionResult::NoChange,
        Value::Int(i) => CoercionResult::Coerced(Value::Bool(*i != 0)),
        Value::Float(f) => CoercionResult::Coerced(Value::Bool(*f != 0.0)),
        Value::String(s) => {
            let lower = s.trim().to_lowercase();
            match lower.as_str() {
                "true" | "1" | "yes" | "on" | "t" | "y" => {
                    CoercionResult::Coerced(Value::Bool(true))
                }
                "false" | "0" | "no" | "off" | "f" | "n" | "" => {
                    CoercionResult::Coerced(Value::Bool(false))
                }
                _ => CoercionResult::Failed,
            }
        }
        Value::Null => CoercionResult::Coerced(Value::Bool(false)),
        _ => CoercionResult::Failed,
    }
}

/// Coerce value to string
fn coerce_to_string(value: &Value) -> CoercionResult {
    match value {
        Value::String(_) => CoercionResult::NoChange,
        Value::Int(i) => CoercionResult::Coerced(Value::String(i.to_string())),
        Value::Float(f) => CoercionResult::Coerced(Value::String(f.to_string())),
        Value::Bool(b) => CoercionResult::Coerced(Value::String(b.to_string())),
        Value::Null => CoercionResult::Coerced(Value::String("null".to_string())),
        _ => CoercionResult::Failed,
    }
}

// ============================================================================
// Apply Coercion (returns new Value or original)
// ============================================================================

/// Apply coercion and return the result value
///
/// Returns the coerced value if successful, or the original value if no coercion needed.
/// Returns None if coercion failed.
pub fn apply_coercion(value: &Value, target: &TypeDescriptor, mode: CoercionMode) -> Option<Value> {
    match coerce_value(value, target, mode) {
        CoercionResult::NoChange => Some(value.clone()),
        CoercionResult::Coerced(v) => Some(v),
        CoercionResult::Failed => None,
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strict_mode_type_mismatch() {
        // String doesn't match Int64 target - should fail in strict mode
        let value = Value::String("123".to_string());
        let result = coerce_value(
            &value,
            &TypeDescriptor::Int64(Default::default()),
            CoercionMode::Strict,
        );
        assert_eq!(result, CoercionResult::Failed);
    }

    #[test]
    fn test_strict_mode_type_match() {
        // Int matches Int64 target - should return NoChange in strict mode
        let value = Value::Int(123);
        let result = coerce_value(
            &value,
            &TypeDescriptor::Int64(Default::default()),
            CoercionMode::Strict,
        );
        assert_eq!(result, CoercionResult::NoChange);
    }

    #[test]
    fn test_strict_mode_int_to_float() {
        // Int is acceptable for Float64 target in strict mode
        let value = Value::Int(42);
        let result = coerce_value(
            &value,
            &TypeDescriptor::Float64(Default::default()),
            CoercionMode::Strict,
        );
        assert_eq!(result, CoercionResult::NoChange);
    }

    #[test]
    fn test_string_to_int() {
        let value = Value::String("123".to_string());
        let result = coerce_value(
            &value,
            &TypeDescriptor::Int64(Default::default()),
            CoercionMode::Lax,
        );
        assert_eq!(result, CoercionResult::Coerced(Value::Int(123)));
    }

    #[test]
    fn test_string_to_int_with_whitespace() {
        let value = Value::String("  456  ".to_string());
        let result = coerce_value(
            &value,
            &TypeDescriptor::Int64(Default::default()),
            CoercionMode::Lax,
        );
        assert_eq!(result, CoercionResult::Coerced(Value::Int(456)));
    }

    #[test]
    fn test_string_to_int_negative() {
        let value = Value::String("-789".to_string());
        let result = coerce_value(
            &value,
            &TypeDescriptor::Int64(Default::default()),
            CoercionMode::Lax,
        );
        assert_eq!(result, CoercionResult::Coerced(Value::Int(-789)));
    }

    #[test]
    fn test_string_to_int_invalid() {
        let value = Value::String("not a number".to_string());
        let result = coerce_value(
            &value,
            &TypeDescriptor::Int64(Default::default()),
            CoercionMode::Lax,
        );
        assert_eq!(result, CoercionResult::Failed);
    }

    #[test]
    fn test_float_to_int() {
        let value = Value::Float(42.0);
        let result = coerce_value(
            &value,
            &TypeDescriptor::Int64(Default::default()),
            CoercionMode::Lax,
        );
        assert_eq!(result, CoercionResult::Coerced(Value::Int(42)));
    }

    #[test]
    fn test_float_to_int_non_integer() {
        let value = Value::Float(42.5);
        let result = coerce_value(
            &value,
            &TypeDescriptor::Int64(Default::default()),
            CoercionMode::Lax,
        );
        assert_eq!(result, CoercionResult::Failed);
    }

    #[test]
    fn test_int_to_float() {
        let value = Value::Int(42);
        let result = coerce_value(
            &value,
            &TypeDescriptor::Float64(Default::default()),
            CoercionMode::Lax,
        );
        assert_eq!(result, CoercionResult::Coerced(Value::Float(42.0)));
    }

    #[test]
    fn test_string_to_float() {
        let value = Value::String("3.14".to_string());
        let result = coerce_value(
            &value,
            &TypeDescriptor::Float64(Default::default()),
            CoercionMode::Lax,
        );
        let CoercionResult::Coerced(Value::Float(f)) = result else {
            unreachable!("Expected coerced float");
        };
        assert!((f - 3.14).abs() < 0.001);
    }

    #[test]
    fn test_string_to_bool_true() {
        for s in &["true", "1", "yes", "on", "t", "y", "TRUE", "Yes", "ON"] {
            let value = Value::String(s.to_string());
            let result = coerce_value(&value, &TypeDescriptor::Bool, CoercionMode::Lax);
            assert_eq!(
                result,
                CoercionResult::Coerced(Value::Bool(true)),
                "Failed for: {}",
                s
            );
        }
    }

    #[test]
    fn test_string_to_bool_false() {
        for s in &[
            "false", "0", "no", "off", "f", "n", "", "FALSE", "No", "OFF",
        ] {
            let value = Value::String(s.to_string());
            let result = coerce_value(&value, &TypeDescriptor::Bool, CoercionMode::Lax);
            assert_eq!(
                result,
                CoercionResult::Coerced(Value::Bool(false)),
                "Failed for: {}",
                s
            );
        }
    }

    #[test]
    fn test_int_to_bool() {
        let value = Value::Int(0);
        let result = coerce_value(&value, &TypeDescriptor::Bool, CoercionMode::Lax);
        assert_eq!(result, CoercionResult::Coerced(Value::Bool(false)));

        let value = Value::Int(1);
        let result = coerce_value(&value, &TypeDescriptor::Bool, CoercionMode::Lax);
        assert_eq!(result, CoercionResult::Coerced(Value::Bool(true)));

        let value = Value::Int(-1);
        let result = coerce_value(&value, &TypeDescriptor::Bool, CoercionMode::Lax);
        assert_eq!(result, CoercionResult::Coerced(Value::Bool(true)));
    }

    #[test]
    fn test_bool_to_int() {
        let value = Value::Bool(true);
        let result = coerce_value(
            &value,
            &TypeDescriptor::Int64(Default::default()),
            CoercionMode::Lax,
        );
        assert_eq!(result, CoercionResult::Coerced(Value::Int(1)));

        let value = Value::Bool(false);
        let result = coerce_value(
            &value,
            &TypeDescriptor::Int64(Default::default()),
            CoercionMode::Lax,
        );
        assert_eq!(result, CoercionResult::Coerced(Value::Int(0)));
    }

    #[test]
    fn test_int_to_string() {
        let value = Value::Int(42);
        let result = coerce_value(
            &value,
            &TypeDescriptor::String(Default::default()),
            CoercionMode::Lax,
        );
        assert_eq!(
            result,
            CoercionResult::Coerced(Value::String("42".to_string()))
        );
    }

    #[test]
    fn test_float_to_string() {
        let value = Value::Float(3.14);
        let result = coerce_value(
            &value,
            &TypeDescriptor::String(Default::default()),
            CoercionMode::Lax,
        );
        let CoercionResult::Coerced(Value::String(s)) = result else {
            unreachable!("Expected coerced string");
        };
        assert!(s.starts_with("3.14"));
    }

    #[test]
    fn test_bool_to_string() {
        let value = Value::Bool(true);
        let result = coerce_value(
            &value,
            &TypeDescriptor::String(Default::default()),
            CoercionMode::Lax,
        );
        assert_eq!(
            result,
            CoercionResult::Coerced(Value::String("true".to_string()))
        );
    }

    #[test]
    fn test_apply_coercion() {
        let value = Value::String("123".to_string());
        let result = apply_coercion(
            &value,
            &TypeDescriptor::Int64(Default::default()),
            CoercionMode::Lax,
        );
        assert_eq!(result, Some(Value::Int(123)));
    }

    #[test]
    fn test_apply_coercion_no_change() {
        let value = Value::Int(42);
        let result = apply_coercion(
            &value,
            &TypeDescriptor::Int64(Default::default()),
            CoercionMode::Lax,
        );
        assert_eq!(result, Some(Value::Int(42)));
    }

    #[test]
    fn test_apply_coercion_failed() {
        let value = Value::String("not a number".to_string());
        let result = apply_coercion(
            &value,
            &TypeDescriptor::Int64(Default::default()),
            CoercionMode::Lax,
        );
        assert_eq!(result, None);
    }

    #[test]
    fn test_coerce_optional() {
        // Null stays null for Optional
        let value = Value::Null;
        let target = TypeDescriptor::Optional(Box::new(TypeDescriptor::Int64(Default::default())));
        let result = coerce_value(&value, &target, CoercionMode::Lax);
        assert_eq!(result, CoercionResult::NoChange);

        // String coerced to Optional<Int>
        let value = Value::String("42".to_string());
        let result = coerce_value(&value, &target, CoercionMode::Lax);
        assert_eq!(result, CoercionResult::Coerced(Value::Int(42)));
    }

    #[test]
    fn test_coerce_list() {
        // List of strings coerced to list of ints
        let value = Value::List(vec![
            Value::String("1".to_string()),
            Value::String("2".to_string()),
            Value::String("3".to_string()),
        ]);
        let target = TypeDescriptor::List {
            items: Box::new(TypeDescriptor::Int64(Default::default())),
            constraints: Default::default(),
        };
        let result = coerce_value(&value, &target, CoercionMode::Lax);
        assert_eq!(
            result,
            CoercionResult::Coerced(Value::List(vec![
                Value::Int(1),
                Value::Int(2),
                Value::Int(3),
            ]))
        );
    }

    #[test]
    fn test_coerce_list_partial_failure() {
        // List with invalid element fails entirely
        let value = Value::List(vec![
            Value::String("1".to_string()),
            Value::String("not a number".to_string()),
        ]);
        let target = TypeDescriptor::List {
            items: Box::new(TypeDescriptor::Int64(Default::default())),
            constraints: Default::default(),
        };
        let result = coerce_value(&value, &target, CoercionMode::Lax);
        assert_eq!(result, CoercionResult::Failed);
    }

    #[test]
    fn test_depth_limit() {
        // Create deeply nested Optional types
        let mut target = TypeDescriptor::Int64(Default::default());
        for _ in 0..(MAX_COERCION_DEPTH + 10) {
            target = TypeDescriptor::Optional(Box::new(target));
        }

        // Should fail due to depth limit, not stack overflow
        let value = Value::String("42".to_string());
        let result = coerce_value(&value, &target, CoercionMode::Lax);
        assert_eq!(result, CoercionResult::Failed);
    }
}
