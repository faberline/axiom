//! Compatibility layer for migration from pydantic_validation to cclab-shield.
//!
//! This module provides type aliases and wrapper types to maintain backward
//! compatibility with the old `pydantic_validation` module while using
//! `cclab-shield` as the underlying implementation.
//!
//! # Migration Guide
//!
//! The old `pydantic_validation` module has been replaced by `cclab-shield`.
//! Most types can be migrated by updating imports:
//!
//! ```text
//! // Old import
//! use pg::pydantic_validation::{ValidationError, EmailValidator};
//!
//! // New import (using compat layer)
//! use pg::compat::{ValidationError, EmailValidator};
//!
//! // Or use cclab-shield directly (recommended)
//! use schema::{ValidationError, ValidationErrors};
//! ```

use schema;

// ============================================================================
// Type Aliases for Backward Compatibility
// ============================================================================

/// Alias for shield's ValidatorMode (was ValidationMode)
pub type ValidationMode = schema::ValidatorMode;

/// Alias for shield's ValidatorContext
pub type ValidatorContext = schema::ValidatorContext;

/// Alias for shield's ValidatorCollection (was ValidationRegistry)
pub type ValidationRegistry = schema::ValidatorCollection;

// ============================================================================
// ValidationError Compatibility Wrapper
// ============================================================================

/// Validation error with location information.
///
/// This is a compatibility wrapper around `schema::ValidationError`.
/// For new code, prefer using `schema::ValidationError` directly.
#[derive(Debug, Clone)]
pub struct ValidationError {
    /// Error location (field path)
    pub loc: Vec<String>,
    /// Error message
    pub msg: String,
    /// Error type
    pub error_type: String,
    /// Input value that caused the error (as string)
    pub input: Option<String>,
}

impl ValidationError {
    /// Create a field validation error.
    pub fn field(field: impl Into<String>, msg: impl Into<String>) -> Self {
        Self {
            loc: vec![field.into()],
            msg: msg.into(),
            error_type: "value_error".to_string(),
            input: None,
        }
    }

    /// Create with a specific error type.
    pub fn with_type(mut self, error_type: impl Into<String>) -> Self {
        self.error_type = error_type.into();
        self
    }

    /// Set the input value.
    pub fn with_input(mut self, input: impl Into<String>) -> Self {
        self.input = Some(input.into());
        self
    }

    /// Create a nested error location.
    pub fn nested(mut self, parent: impl Into<String>) -> Self {
        self.loc.insert(0, parent.into());
        self
    }

    /// Convert to shield ValidationError
    pub fn to_shield(&self) -> schema::ValidationError {
        // Map error_type string to shield ErrorType
        let error_type = match self.error_type.as_str() {
            s if s.starts_with("type_error") => schema::ErrorType::TypeError,
            s if s.starts_with("value_error") => schema::ErrorType::ValueError,
            "missing" | "missing_error" => schema::ErrorType::Missing,
            "extra_forbidden" => schema::ErrorType::ExtraForbidden,
            s if s.starts_with("format_error") => schema::ErrorType::FormatError,
            _ => schema::ErrorType::ValueError, // Default fallback
        };

        schema::ValidationError::new(
            self.loc.first().cloned().unwrap_or_default(),
            self.loc.get(1..).map(|s| s.join(".")).unwrap_or_default(),
            self.msg.clone(),
            error_type,
        )
    }
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.loc.join("."), self.msg)
    }
}

impl From<schema::ValidationError> for ValidationError {
    fn from(e: schema::ValidationError) -> Self {
        // Build loc from location and field, splitting field by '.' if present
        let mut loc = vec![e.location.clone()];
        if !e.field.is_empty() {
            // Split field by '.' to preserve nested structure
            for part in e.field.split('.') {
                if !part.is_empty() {
                    loc.push(part.to_string());
                }
            }
        }
        Self {
            loc,
            msg: e.message,
            error_type: e.error_type.to_string(),
            input: None,
        }
    }
}

impl From<ValidationError> for schema::ValidationError {
    fn from(e: ValidationError) -> Self {
        e.to_shield()
    }
}

// ============================================================================
// ValidationErrors Compatibility Wrapper
// ============================================================================

/// Collection of validation errors.
///
/// This is a compatibility wrapper that provides the old API while
/// delegating to `schema::ValidationErrors`.
#[derive(Debug, Clone, Default)]
pub struct ValidationErrors {
    /// List of errors
    pub errors: Vec<ValidationError>,
}

impl ValidationErrors {
    /// Create empty validation errors.
    pub fn new() -> Self {
        Self { errors: Vec::new() }
    }

    /// Add an error.
    pub fn add(&mut self, error: ValidationError) {
        self.errors.push(error);
    }

    /// Check if there are any errors.
    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }

    /// Get error count.
    pub fn len(&self) -> usize {
        self.errors.len()
    }

    /// Convert to Result (old signature with value).
    ///
    /// **Deprecated**: Use `into_result_unit()` and handle the value separately.
    pub fn into_result<T>(self, value: T) -> crate::Result<T> {
        if self.is_empty() {
            Ok(value)
        } else {
            Err(crate::DataBridgeError::Validation(self.to_string()))
        }
    }

    /// Convert to Result without value (new signature).
    pub fn into_result_unit(self) -> Result<(), Self> {
        if self.is_empty() {
            Ok(())
        } else {
            Err(self)
        }
    }

    /// Convert to shield ValidationErrors
    pub fn to_shield(&self) -> schema::ValidationErrors {
        let mut errors = schema::ValidationErrors::new();
        for e in &self.errors {
            errors.add(e.to_shield());
        }
        errors
    }
}

impl std::fmt::Display for ValidationErrors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msgs: Vec<String> = self.errors.iter().map(|e| e.to_string()).collect();
        write!(f, "Validation failed: {}", msgs.join("; "))
    }
}

impl From<schema::ValidationErrors> for ValidationErrors {
    fn from(e: schema::ValidationErrors) -> Self {
        Self {
            errors: e.errors.into_iter().map(ValidationError::from).collect(),
        }
    }
}

impl From<ValidationErrors> for schema::ValidationErrors {
    fn from(e: ValidationErrors) -> Self {
        e.to_shield()
    }
}

// ============================================================================
// Field Validator Config (Compatibility)
// ============================================================================

/// Field validator configuration.
#[derive(Debug, Clone)]
pub struct FieldValidatorConfig {
    /// Field name to validate
    pub field_name: String,
    /// Validation mode
    pub mode: ValidationMode,
    /// Check fields for validation context
    pub check_fields: bool,
    /// Validator ID (for tracking)
    pub validator_id: String,
}

impl FieldValidatorConfig {
    /// Create a new field validator config.
    pub fn new(field_name: impl Into<String>) -> Self {
        let field_name = field_name.into();
        Self {
            validator_id: format!("validator_{}", field_name),
            field_name,
            mode: ValidationMode::default(),
            check_fields: true,
        }
    }

    /// Set validation mode.
    pub fn mode(mut self, mode: ValidationMode) -> Self {
        self.mode = mode;
        self
    }

    /// Set check_fields flag.
    pub fn check_fields(mut self, check: bool) -> Self {
        self.check_fields = check;
        self
    }
}

// ============================================================================
// Model Validator Config (Compatibility)
// ============================================================================

/// Model validator configuration.
#[derive(Debug, Clone)]
pub struct ModelValidatorConfig {
    /// Validation mode
    pub mode: ValidationMode,
    /// Validator ID
    pub validator_id: String,
}

impl Default for ModelValidatorConfig {
    fn default() -> Self {
        Self {
            mode: ValidationMode::After,
            validator_id: "model_validator".to_string(),
        }
    }
}

// ============================================================================
// Computed Field Config (Compatibility)
// ============================================================================

/// Computed field configuration.
#[derive(Debug, Clone)]
pub struct ComputedFieldConfig {
    /// Field name
    pub field_name: String,
    /// Whether to include in serialization
    pub repr: bool,
    /// Return type description
    pub return_type: String,
}

impl ComputedFieldConfig {
    /// Create a new computed field config.
    pub fn new(field_name: impl Into<String>) -> Self {
        Self {
            field_name: field_name.into(),
            repr: true,
            return_type: "string".to_string(),
        }
    }

    /// Set repr flag.
    pub fn repr(mut self, repr: bool) -> Self {
        self.repr = repr;
        self
    }

    /// Set return type.
    pub fn return_type(mut self, return_type: impl Into<String>) -> Self {
        self.return_type = return_type.into();
        self
    }
}

// ============================================================================
// Built-in Validators
// ============================================================================

/// Email validator.
pub struct EmailValidator {
    field_name: String,
}

impl EmailValidator {
    /// Create a new email validator.
    pub fn new(field_name: impl Into<String>) -> Self {
        Self {
            field_name: field_name.into(),
        }
    }

    /// Validate a string value.
    pub fn validate_string(&self, value: &str) -> Result<String, ValidationError> {
        if !value.contains('@') || !value.contains('.') {
            return Err(
                ValidationError::field(&self.field_name, "Invalid email format")
                    .with_type("value_error.email")
                    .with_input(value),
            );
        }

        let parts: Vec<&str> = value.split('@').collect();
        if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
            return Err(
                ValidationError::field(&self.field_name, "Invalid email format")
                    .with_type("value_error.email")
                    .with_input(value),
            );
        }

        Ok(value.to_lowercase())
    }
}

/// URL validator.
pub struct UrlValidator {
    field_name: String,
    allowed_schemes: Vec<String>,
}

impl UrlValidator {
    /// Create a new URL validator.
    pub fn new(field_name: impl Into<String>) -> Self {
        Self {
            field_name: field_name.into(),
            allowed_schemes: vec!["http".to_string(), "https".to_string()],
        }
    }

    /// Set allowed URL schemes.
    pub fn allowed_schemes(mut self, schemes: Vec<String>) -> Self {
        self.allowed_schemes = schemes;
        self
    }

    /// Validate a string value.
    pub fn validate_string(&self, value: &str) -> Result<String, ValidationError> {
        let has_scheme = self
            .allowed_schemes
            .iter()
            .any(|s| value.starts_with(&format!("{}://", s)));

        if !has_scheme {
            return Err(ValidationError::field(
                &self.field_name,
                format!("URL must start with one of: {:?}", self.allowed_schemes),
            )
            .with_type("value_error.url")
            .with_input(value));
        }

        Ok(value.to_string())
    }
}

/// Length validator.
pub struct LengthValidator {
    field_name: String,
    min_length: Option<usize>,
    max_length: Option<usize>,
}

impl LengthValidator {
    /// Create a new length validator.
    pub fn new(field_name: impl Into<String>) -> Self {
        Self {
            field_name: field_name.into(),
            min_length: None,
            max_length: None,
        }
    }

    /// Set minimum length.
    pub fn min(mut self, min: usize) -> Self {
        self.min_length = Some(min);
        self
    }

    /// Set maximum length.
    pub fn max(mut self, max: usize) -> Self {
        self.max_length = Some(max);
        self
    }

    /// Validate a string value.
    pub fn validate_string(&self, value: &str) -> Result<String, ValidationError> {
        let len = value.len();

        if let Some(min) = self.min_length {
            if len < min {
                return Err(ValidationError::field(
                    &self.field_name,
                    format!("String must be at least {} characters", min),
                )
                .with_type("value_error.string.min_length")
                .with_input(value));
            }
        }

        if let Some(max) = self.max_length {
            if len > max {
                return Err(ValidationError::field(
                    &self.field_name,
                    format!("String must be at most {} characters", max),
                )
                .with_type("value_error.string.max_length")
                .with_input(value));
            }
        }

        Ok(value.to_string())
    }
}

/// Range validator for numbers.
pub struct RangeValidator {
    field_name: String,
    min: Option<f64>,
    max: Option<f64>,
    exclusive_min: bool,
    exclusive_max: bool,
}

impl RangeValidator {
    /// Create a new range validator.
    pub fn new(field_name: impl Into<String>) -> Self {
        Self {
            field_name: field_name.into(),
            min: None,
            max: None,
            exclusive_min: false,
            exclusive_max: false,
        }
    }

    /// Set minimum value (inclusive).
    pub fn ge(mut self, min: f64) -> Self {
        self.min = Some(min);
        self.exclusive_min = false;
        self
    }

    /// Set minimum value (exclusive).
    pub fn gt(mut self, min: f64) -> Self {
        self.min = Some(min);
        self.exclusive_min = true;
        self
    }

    /// Set maximum value (inclusive).
    pub fn le(mut self, max: f64) -> Self {
        self.max = Some(max);
        self.exclusive_max = false;
        self
    }

    /// Set maximum value (exclusive).
    pub fn lt(mut self, max: f64) -> Self {
        self.max = Some(max);
        self.exclusive_max = true;
        self
    }

    /// Validate an integer value.
    pub fn validate_int(&self, value: i64) -> Result<i64, ValidationError> {
        self.validate_float(value as f64)?;
        Ok(value)
    }

    /// Validate a float value.
    pub fn validate_float(&self, value: f64) -> Result<f64, ValidationError> {
        if let Some(min) = self.min {
            let valid = if self.exclusive_min {
                value > min
            } else {
                value >= min
            };
            if !valid {
                let op = if self.exclusive_min { ">" } else { ">=" };
                return Err(ValidationError::field(
                    &self.field_name,
                    format!("Value must be {} {}", op, min),
                )
                .with_type("value_error.number.not_ge")
                .with_input(value.to_string()));
            }
        }

        if let Some(max) = self.max {
            let valid = if self.exclusive_max {
                value < max
            } else {
                value <= max
            };
            if !valid {
                let op = if self.exclusive_max { "<" } else { "<=" };
                return Err(ValidationError::field(
                    &self.field_name,
                    format!("Value must be {} {}", op, max),
                )
                .with_type("value_error.number.not_le")
                .with_input(value.to_string()));
            }
        }

        Ok(value)
    }
}

/// Pattern validator for string matching.
pub struct PatternValidator {
    field_name: String,
    pattern: String,
}

impl PatternValidator {
    /// Create a new pattern validator.
    pub fn new(field_name: impl Into<String>, pattern: impl Into<String>) -> Self {
        Self {
            field_name: field_name.into(),
            pattern: pattern.into(),
        }
    }

    /// Validate a string value.
    pub fn validate_string(&self, value: &str) -> Result<String, ValidationError> {
        if !value.contains(&self.pattern) && !self.pattern.is_empty() {
            return Err(ValidationError::field(
                &self.field_name,
                format!("String does not match pattern: {}", self.pattern),
            )
            .with_type("value_error.string.pattern")
            .with_input(value));
        }
        Ok(value.to_string())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_email_validator() {
        let validator = EmailValidator::new("email");

        assert!(validator.validate_string("test@example.com").is_ok());
        assert!(validator.validate_string("invalid").is_err());
        assert!(validator.validate_string("@example.com").is_err());
        assert!(validator.validate_string("test@").is_err());
    }

    #[test]
    fn test_email_normalizes_to_lowercase() {
        let validator = EmailValidator::new("email");
        let result = validator.validate_string("Test@Example.COM").unwrap();
        assert_eq!(result, "test@example.com");
    }

    #[test]
    fn test_length_validator() {
        let validator = LengthValidator::new("name").min(3).max(10);

        assert!(validator.validate_string("hello").is_ok());
        assert!(validator.validate_string("ab").is_err());
        assert!(validator.validate_string("this is too long").is_err());
    }

    #[test]
    fn test_range_validator() {
        let validator = RangeValidator::new("age").ge(0.0).le(120.0);

        assert!(validator.validate_int(25).is_ok());
        assert!(validator.validate_int(-1).is_err());
        assert!(validator.validate_int(150).is_err());
    }

    #[test]
    fn test_range_validator_exclusive() {
        let validator = RangeValidator::new("score").gt(0.0).lt(100.0);

        assert!(validator.validate_float(50.0).is_ok());
        assert!(validator.validate_float(0.0).is_err());
        assert!(validator.validate_float(100.0).is_err());
    }

    #[test]
    fn test_url_validator() {
        let validator = UrlValidator::new("website");

        assert!(validator.validate_string("https://example.com").is_ok());
        assert!(validator.validate_string("http://example.com").is_ok());
        assert!(validator.validate_string("ftp://example.com").is_err());
        assert!(validator.validate_string("example.com").is_err());
    }

    #[test]
    fn test_validation_errors() {
        let mut errors = ValidationErrors::new();
        assert!(errors.is_empty());

        errors.add(ValidationError::field("name", "Required"));
        errors.add(ValidationError::field("email", "Invalid"));

        assert_eq!(errors.len(), 2);
        assert!(!errors.is_empty());
    }

    #[test]
    fn test_validation_errors_into_result() {
        let errors = ValidationErrors::new();
        let result: crate::Result<i32> = errors.into_result(42);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);

        let mut errors = ValidationErrors::new();
        errors.add(ValidationError::field("test", "error"));
        let result: crate::Result<i32> = errors.into_result(42);
        assert!(result.is_err());
    }

    #[test]
    fn test_pattern_validator() {
        let validator = PatternValidator::new("code", "ABC");

        assert!(validator.validate_string("ABC123").is_ok());
        assert!(validator.validate_string("XYZABC").is_ok());
        assert!(validator.validate_string("XYZ123").is_err());
    }

    #[test]
    fn test_pattern_validator_empty_pattern() {
        let validator = PatternValidator::new("code", "");
        // Empty pattern should match anything
        assert!(validator.validate_string("anything").is_ok());
    }

    #[test]
    fn test_url_validator_custom_schemes() {
        let validator =
            UrlValidator::new("link").allowed_schemes(vec!["ftp".to_string(), "sftp".to_string()]);

        assert!(validator.validate_string("ftp://example.com").is_ok());
        assert!(validator.validate_string("sftp://example.com").is_ok());
        assert!(validator.validate_string("https://example.com").is_err());
    }

    #[test]
    fn test_validation_error_to_shield_conversion() {
        // Test value_error mapping
        let error = ValidationError::field("email", "Invalid format")
            .with_type("value_error.email")
            .with_input("bad@");
        let shield_error = error.to_shield();
        assert_eq!(shield_error.error_type.to_string(), "value_error");

        // Test type_error mapping
        let error =
            ValidationError::field("age", "Must be integer").with_type("type_error.integer");
        let shield_error = error.to_shield();
        assert_eq!(shield_error.error_type.to_string(), "type_error");

        // Test missing mapping
        let error = ValidationError::field("name", "Required").with_type("missing");
        let shield_error = error.to_shield();
        assert_eq!(shield_error.error_type.to_string(), "missing");
    }

    #[test]
    fn test_validation_error_from_shield_conversion() {
        let shield_error = schema::ValidationError::new(
            "body".to_string(),
            "user.email".to_string(),
            "Invalid email".to_string(),
            schema::ErrorType::FormatError,
        );

        let compat_error = ValidationError::from(shield_error);
        assert_eq!(compat_error.loc, vec!["body", "user", "email"]);
        assert_eq!(compat_error.msg, "Invalid email");
        assert_eq!(compat_error.error_type, "format_error");
    }

    #[test]
    fn test_validation_error_into_shield() {
        let error =
            ValidationError::field("name", "Too short").with_type("value_error.string.min_length");

        // Test Into trait
        let shield_error: schema::ValidationError = error.into();
        assert_eq!(shield_error.location, "name");
        assert_eq!(shield_error.message, "Too short");
    }

    #[test]
    fn test_validation_errors_into_shield() {
        let mut errors = ValidationErrors::new();
        errors.add(ValidationError::field("name", "Required").with_type("missing"));
        errors.add(ValidationError::field("email", "Invalid").with_type("format_error"));

        // Test Into trait
        let shield_errors: schema::ValidationErrors = errors.into();
        assert_eq!(shield_errors.len(), 2);
    }

    #[test]
    fn test_nested_error_location() {
        let error = ValidationError::field("city", "Required")
            .nested("address")
            .nested("user");

        assert_eq!(error.loc, vec!["user", "address", "city"]);

        // Convert to shield and back
        let shield_error = error.to_shield();
        assert_eq!(shield_error.location, "user");
        assert_eq!(shield_error.field, "address.city");
    }
}
