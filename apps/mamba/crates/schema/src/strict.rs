//! Strict mode validation
//!
//! Provides strict type checking without automatic coercion.
//! In strict mode, "123" is not valid for an int field.

use crate::errors::ValidationError;
use crate::types::Value;

// ============================================================================
// Strict Mode Configuration
// ============================================================================

/// Strict mode settings
#[derive(Debug, Clone, Copy, Default)]
pub struct StrictMode {
    /// Require exact types (no int -> float coercion)
    pub strict_types: bool,
    /// Require strings for string fields (no number -> string coercion)
    pub strict_strings: bool,
    /// Require booleans for bool fields (no "true" -> true coercion)
    pub strict_bools: bool,
    /// Require numbers for numeric fields (no "123" -> 123 coercion)
    pub strict_numbers: bool,
}

impl StrictMode {
    /// Create a new strict mode (all strict)
    pub fn strict() -> Self {
        Self {
            strict_types: true,
            strict_strings: true,
            strict_bools: true,
            strict_numbers: true,
        }
    }

    /// Create a lenient mode (no strict checks)
    pub fn lenient() -> Self {
        Self::default()
    }

    /// Enable strict type checking
    pub fn types(mut self, strict: bool) -> Self {
        self.strict_types = strict;
        self
    }

    /// Enable strict string checking
    pub fn strings(mut self, strict: bool) -> Self {
        self.strict_strings = strict;
        self
    }

    /// Enable strict boolean checking
    pub fn bools(mut self, strict: bool) -> Self {
        self.strict_bools = strict;
        self
    }

    /// Enable strict number checking
    pub fn numbers(mut self, strict: bool) -> Self {
        self.strict_numbers = strict;
        self
    }

    /// Check if value is strictly valid as a string
    pub fn check_string(&self, value: &Value) -> Result<(), ValidationError> {
        if !self.strict_strings {
            return Ok(());
        }

        match value {
            Value::String(_) => Ok(()),
            _ => Err(ValidationError::type_error(
                "strict".to_string(),
                "".to_string(),
                format!("Strict mode: expected string, got {}", value.type_name()),
            )),
        }
    }

    /// Check if value is strictly valid as a boolean
    pub fn check_bool(&self, value: &Value) -> Result<(), ValidationError> {
        if !self.strict_bools {
            return Ok(());
        }

        match value {
            Value::Bool(_) => Ok(()),
            _ => Err(ValidationError::type_error(
                "strict".to_string(),
                "".to_string(),
                format!("Strict mode: expected boolean, got {}", value.type_name()),
            )),
        }
    }

    /// Check if value is strictly valid as an integer
    pub fn check_int(&self, value: &Value) -> Result<(), ValidationError> {
        if !self.strict_numbers {
            return Ok(());
        }

        match value {
            Value::Int(_) => Ok(()),
            _ => Err(ValidationError::type_error(
                "strict".to_string(),
                "".to_string(),
                format!("Strict mode: expected integer, got {}", value.type_name()),
            )),
        }
    }

    /// Check if value is strictly valid as a float
    pub fn check_float(&self, value: &Value) -> Result<(), ValidationError> {
        if !self.strict_numbers {
            return Ok(());
        }

        match value {
            Value::Float(_) => Ok(()),
            // In strict types mode, don't allow int -> float
            Value::Int(_) if !self.strict_types => Ok(()),
            _ => Err(ValidationError::type_error(
                "strict".to_string(),
                "".to_string(),
                format!("Strict mode: expected number, got {}", value.type_name()),
            )),
        }
    }

    /// Check if value is strictly valid as a number (int or float)
    pub fn check_number(&self, value: &Value) -> Result<(), ValidationError> {
        if !self.strict_numbers {
            return Ok(());
        }

        match value {
            Value::Int(_) | Value::Float(_) => Ok(()),
            _ => Err(ValidationError::type_error(
                "strict".to_string(),
                "".to_string(),
                format!("Strict mode: expected number, got {}", value.type_name()),
            )),
        }
    }
}

// ============================================================================
// Strict Validation Result
// ============================================================================

/// Result of strict validation with coercion info
#[derive(Debug)]
pub struct StrictResult<T> {
    /// The result value
    pub value: T,
    /// Whether coercion was applied
    pub coerced: bool,
    /// Description of coercion if applied
    pub coercion: Option<String>,
}

impl<T> StrictResult<T> {
    /// Create a new strict result without coercion
    pub fn exact(value: T) -> Self {
        Self {
            value,
            coerced: false,
            coercion: None,
        }
    }

    /// Create a new strict result with coercion
    pub fn coerced(value: T, description: impl Into<String>) -> Self {
        Self {
            value,
            coerced: true,
            coercion: Some(description.into()),
        }
    }

    /// Map the value
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> StrictResult<U> {
        StrictResult {
            value: f(self.value),
            coerced: self.coerced,
            coercion: self.coercion,
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strict_mode_default() {
        let mode = StrictMode::default();
        assert!(!mode.strict_types);
        assert!(!mode.strict_strings);
        assert!(!mode.strict_bools);
        assert!(!mode.strict_numbers);
    }

    #[test]
    fn test_strict_mode_strict() {
        let mode = StrictMode::strict();
        assert!(mode.strict_types);
        assert!(mode.strict_strings);
        assert!(mode.strict_bools);
        assert!(mode.strict_numbers);
    }

    #[test]
    fn test_check_string_lenient() {
        let mode = StrictMode::lenient();
        assert!(mode.check_string(&Value::Int(123)).is_ok());
    }

    #[test]
    fn test_check_string_strict() {
        let mode = StrictMode::strict();
        assert!(mode
            .check_string(&Value::String("hello".to_string()))
            .is_ok());
        assert!(mode.check_string(&Value::Int(123)).is_err());
    }

    #[test]
    fn test_check_bool_lenient() {
        let mode = StrictMode::lenient();
        assert!(mode.check_bool(&Value::String("true".to_string())).is_ok());
    }

    #[test]
    fn test_check_bool_strict() {
        let mode = StrictMode::strict();
        assert!(mode.check_bool(&Value::Bool(true)).is_ok());
        assert!(mode.check_bool(&Value::String("true".to_string())).is_err());
    }

    #[test]
    fn test_check_int_lenient() {
        let mode = StrictMode::lenient();
        assert!(mode.check_int(&Value::String("123".to_string())).is_ok());
    }

    #[test]
    fn test_check_int_strict() {
        let mode = StrictMode::strict();
        assert!(mode.check_int(&Value::Int(123)).is_ok());
        assert!(mode.check_int(&Value::String("123".to_string())).is_err());
        assert!(mode.check_int(&Value::Float(123.0)).is_err());
    }

    #[test]
    fn test_check_float_strict_types() {
        // With strict_types, int -> float is not allowed
        let mode = StrictMode::strict();
        assert!(mode.check_float(&Value::Float(1.5)).is_ok());
        assert!(mode.check_float(&Value::Int(1)).is_err());

        // Without strict_types, int -> float is allowed
        let mode = StrictMode::lenient().numbers(true);
        assert!(mode.check_float(&Value::Float(1.5)).is_ok());
        assert!(mode.check_float(&Value::Int(1)).is_ok());
    }

    #[test]
    fn test_strict_result() {
        let exact = StrictResult::exact(42);
        assert!(!exact.coerced);
        assert!(exact.coercion.is_none());

        let coerced = StrictResult::coerced(42, "string to int");
        assert!(coerced.coerced);
        assert_eq!(coerced.coercion, Some("string to int".to_string()));
    }

    #[test]
    fn test_strict_result_map() {
        let result = StrictResult::coerced(42, "coercion");
        let mapped = result.map(|v| v * 2);

        assert_eq!(mapped.value, 84);
        assert!(mapped.coerced);
        assert_eq!(mapped.coercion, Some("coercion".to_string()));
    }
}
