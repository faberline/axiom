//! Validation error types
//!
//! This module defines error types for validation failures.

use std::fmt;

// ============================================================================
// Validation Result
// ============================================================================

/// Validation result type
pub type ValidationResult<T> = Result<T, ValidationErrors>;

// ============================================================================
// Validation Errors Collection
// ============================================================================

/// Collection of validation errors
///
/// This type holds multiple validation errors that occurred during validation.
/// It provides a Pydantic-compatible error format for easy integration with
/// Python code.
#[derive(Debug, Clone, Default)]
pub struct ValidationErrors {
    /// List of individual validation errors
    pub errors: Vec<ValidationError>,
}

impl ValidationErrors {
    /// Create a new empty validation errors collection
    pub fn new() -> Self {
        Self { errors: Vec::new() }
    }

    /// Check if there are any errors
    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }

    /// Get the number of errors
    pub fn len(&self) -> usize {
        self.errors.len()
    }

    /// Add a validation error to the collection
    pub fn add(&mut self, error: ValidationError) {
        self.errors.push(error);
    }

    /// Add multiple validation errors
    pub fn extend(&mut self, errors: impl IntoIterator<Item = ValidationError>) {
        self.errors.extend(errors);
    }

    /// Merge another ValidationErrors into this one
    pub fn merge(&mut self, other: ValidationErrors) {
        self.errors.extend(other.errors);
    }

    /// Convert to Result - Ok if no errors, Err if there are errors
    pub fn into_result(self) -> ValidationResult<()> {
        if self.is_empty() {
            Ok(())
        } else {
            Err(self)
        }
    }

    /// Get errors as a slice
    pub fn as_slice(&self) -> &[ValidationError] {
        &self.errors
    }
}

impl fmt::Display for ValidationErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} validation error(s)", self.errors.len())
    }
}

impl std::error::Error for ValidationErrors {}

// ============================================================================
// Single Validation Error
// ============================================================================

/// A single validation error
///
/// This struct represents a single field validation error with information
/// about where the error occurred and what went wrong.
#[derive(Debug, Clone)]
pub struct ValidationError {
    /// Location of the error (e.g., "body", "query", "path", "field.nested")
    pub location: String,

    /// Field name or path (e.g., "user_id", "address.city")
    pub field: String,

    /// Human-readable error message
    pub message: String,

    /// Error type classification
    pub error_type: ErrorType,
}

impl ValidationError {
    /// Create a new validation error
    pub fn new(location: String, field: String, message: String, error_type: ErrorType) -> Self {
        Self {
            location,
            field,
            message,
            error_type,
        }
    }

    /// Create a type error
    pub fn type_error(location: String, field: String, message: String) -> Self {
        Self::new(location, field, message, ErrorType::TypeError)
    }

    /// Create a value error
    pub fn value_error(location: String, field: String, message: String) -> Self {
        Self::new(location, field, message, ErrorType::ValueError)
    }

    /// Create a missing field error
    pub fn missing_error(location: String, field: String) -> Self {
        Self::new(
            location,
            field,
            "Field required".to_string(),
            ErrorType::Missing,
        )
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} ({}): {} [{}]",
            self.location, self.field, self.message, self.error_type
        )
    }
}

// ============================================================================
// Error Type Classification
// ============================================================================

/// Classification of validation errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorType {
    /// Type mismatch error (e.g., expected string, got integer)
    TypeError,

    /// Value constraint violation (e.g., string too long, number out of range)
    ValueError,

    /// Required field missing
    Missing,

    /// Extra field not allowed
    ExtraForbidden,

    /// Invalid format (e.g., invalid email, malformed UUID)
    FormatError,
}

impl fmt::Display for ErrorType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TypeError => write!(f, "type_error"),
            Self::ValueError => write!(f, "value_error"),
            Self::Missing => write!(f, "missing"),
            Self::ExtraForbidden => write!(f, "extra_forbidden"),
            Self::FormatError => write!(f, "format_error"),
        }
    }
}

// ============================================================================
// Validation Context
// ============================================================================

/// Context for validation (tracks current location in nested structures)
#[derive(Debug, Clone)]
pub struct ValidationContext {
    /// Current path (e.g., "body", "body.user", "body.user.address")
    pub path: Vec<String>,
}

impl ValidationContext {
    /// Create a new validation context
    pub fn new() -> Self {
        Self { path: Vec::new() }
    }

    /// Create a validation context with an initial location
    pub fn with_location(location: &str) -> Self {
        Self {
            path: vec![location.to_string()],
        }
    }

    /// Push a field name onto the path
    pub fn push(&mut self, field: &str) {
        self.path.push(field.to_string());
    }

    /// Pop a field name from the path
    pub fn pop(&mut self) {
        self.path.pop();
    }

    /// Get the current path as a string (e.g., "body.user.address")
    pub fn current_path(&self) -> String {
        self.path.join(".")
    }

    /// Get the current location (first element of path)
    pub fn location(&self) -> String {
        self.path.first().cloned().unwrap_or_default()
    }

    /// Get the current field (everything after location)
    pub fn field(&self) -> String {
        if self.path.len() > 1 {
            self.path[1..].join(".")
        } else {
            String::new()
        }
    }
}

impl Default for ValidationContext {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_errors_empty() {
        let errors = ValidationErrors::new();
        assert!(errors.is_empty());
        assert_eq!(errors.len(), 0);
    }

    #[test]
    fn test_validation_errors_add() {
        let mut errors = ValidationErrors::new();
        errors.add(ValidationError::type_error(
            "body".to_string(),
            "age".to_string(),
            "Expected integer".to_string(),
        ));
        assert!(!errors.is_empty());
        assert_eq!(errors.len(), 1);
    }

    #[test]
    fn test_validation_context() {
        let mut ctx = ValidationContext::new();
        assert_eq!(ctx.current_path(), "");

        ctx.push("body");
        assert_eq!(ctx.current_path(), "body");
        assert_eq!(ctx.location(), "body");
        assert_eq!(ctx.field(), "");

        ctx.push("user");
        assert_eq!(ctx.current_path(), "body.user");
        assert_eq!(ctx.location(), "body");
        assert_eq!(ctx.field(), "user");

        ctx.push("name");
        assert_eq!(ctx.current_path(), "body.user.name");
        assert_eq!(ctx.field(), "user.name");

        ctx.pop();
        assert_eq!(ctx.current_path(), "body.user");
    }

    #[test]
    fn test_error_type_display() {
        assert_eq!(ErrorType::TypeError.to_string(), "type_error");
        assert_eq!(ErrorType::ValueError.to_string(), "value_error");
        assert_eq!(ErrorType::Missing.to_string(), "missing");
    }
}
