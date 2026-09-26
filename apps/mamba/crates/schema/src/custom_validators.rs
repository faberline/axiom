//! Custom validator support for field-level and model-level validation
//!
//! This module provides a trait-based system for custom validators,
//! similar to Pydantic's `@field_validator` and `@model_validator`.
//!
//! # Example (Rust)
//!
//! ```rust,ignore
//! use schema::custom_validators::{FieldValidator, ValidatorMode};
//! use schema::{Value, ValidationResult, ValidationErrors, ValidationError, ErrorType};
//!
//! struct EndDateValidator;
//!
//! impl FieldValidator for EndDateValidator {
//!     fn field_name(&self) -> &str { "end_date" }
//!     fn mode(&self) -> ValidatorMode { ValidatorMode::After }
//!
//!     fn validate(&self, value: &Value, context: &ValidatorContext) -> ValidationResult<Value> {
//!         // Access other fields via context.get_field("start_date")
//!         if let Some(start) = context.get_field("start_date") {
//!             // Compare dates...
//!         }
//!         Ok(value.clone())
//!     }
//! }
//! ```

use crate::errors::{ErrorType, ValidationError, ValidationErrors, ValidationResult};
use crate::types::Value;
use std::collections::HashMap;
use std::sync::Arc;

// ============================================================================
// Validator Mode
// ============================================================================

/// When the validator runs relative to type coercion
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ValidatorMode {
    /// Run before type coercion (receives raw input)
    Before,
    /// Run after type coercion (receives typed value) - default
    #[default]
    After,
    /// Wrap mode - receives raw input, controls coercion
    Wrap,
}

// ============================================================================
// Validator Context
// ============================================================================

/// Context passed to validators with access to other fields
#[derive(Debug, Clone, Default)]
pub struct ValidatorContext {
    /// All field values (for model validators or cross-field access)
    fields: HashMap<String, Value>,
    /// Current field path (e.g., "user.address.city")
    path: Vec<String>,
    /// Location in request (e.g., "body", "query")
    location: String,
    /// Custom metadata
    metadata: HashMap<String, String>,
}

impl ValidatorContext {
    /// Create a new empty context
    pub fn new() -> Self {
        Self::default()
    }

    /// Create context with location
    pub fn with_location(location: impl Into<String>) -> Self {
        Self {
            location: location.into(),
            ..Default::default()
        }
    }

    /// Get a field value by name
    pub fn get_field(&self, name: &str) -> Option<&Value> {
        self.fields.get(name)
    }

    /// Set a field value
    pub fn set_field(&mut self, name: impl Into<String>, value: Value) {
        self.fields.insert(name.into(), value);
    }

    /// Get all fields
    pub fn fields(&self) -> &HashMap<String, Value> {
        &self.fields
    }

    /// Get current field path
    pub fn current_path(&self) -> String {
        self.path.join(".")
    }

    /// Push to field path
    pub fn push_path(&mut self, segment: impl Into<String>) {
        self.path.push(segment.into());
    }

    /// Pop from field path
    pub fn pop_path(&mut self) {
        self.path.pop();
    }

    /// Get location
    pub fn location(&self) -> &str {
        &self.location
    }

    /// Set metadata
    pub fn set_metadata(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
    }

    /// Get metadata
    pub fn get_metadata(&self, key: &str) -> Option<&str> {
        self.metadata.get(key).map(|s| s.as_str())
    }
}

// ============================================================================
// Field Validator Trait
// ============================================================================

/// Trait for field-level validators (like Pydantic's @field_validator)
///
/// Field validators validate a single field and can optionally transform the value.
pub trait FieldValidator: Send + Sync {
    /// Name of the field this validator applies to
    fn field_name(&self) -> &str;

    /// When to run this validator (before/after type coercion)
    fn mode(&self) -> ValidatorMode {
        ValidatorMode::After
    }

    /// Validate and optionally transform the field value
    ///
    /// # Arguments
    /// * `value` - The field value to validate
    /// * `context` - Context with access to other fields
    ///
    /// # Returns
    /// * `Ok(Value)` - Validated (and optionally transformed) value
    /// * `Err(ValidationErrors)` - Validation errors
    fn validate(&self, value: &Value, context: &ValidatorContext) -> ValidationResult<Value>;

    /// Whether this validator is async
    fn is_async(&self) -> bool {
        false
    }
}

// ============================================================================
// Model Validator Trait
// ============================================================================

/// Trait for model-level validators (like Pydantic's @model_validator)
///
/// Model validators validate the entire model and can access all fields.
pub trait ModelValidator: Send + Sync {
    /// When to run this validator (before/after field validation)
    fn mode(&self) -> ValidatorMode {
        ValidatorMode::After
    }

    /// Validate and optionally transform the entire model
    ///
    /// # Arguments
    /// * `value` - The model (Object) value
    /// * `context` - Context with all field values
    ///
    /// # Returns
    /// * `Ok(Value)` - Validated (and optionally transformed) model
    /// * `Err(ValidationErrors)` - Validation errors
    fn validate(&self, value: &Value, context: &ValidatorContext) -> ValidationResult<Value>;

    /// Whether this validator is async
    fn is_async(&self) -> bool {
        false
    }
}

// ============================================================================
// Boxed Validators for Dynamic Dispatch
// ============================================================================

/// Type alias for boxed field validator
pub type BoxedFieldValidator = Arc<dyn FieldValidator>;

/// Type alias for boxed model validator
pub type BoxedModelValidator = Arc<dyn ModelValidator>;

// ============================================================================
// Validator Collection
// ============================================================================

/// Collection of validators for a model/type
#[derive(Default, Clone)]
pub struct ValidatorCollection {
    /// Field validators grouped by field name
    field_validators: HashMap<String, Vec<BoxedFieldValidator>>,
    /// Model validators (run on entire object)
    model_validators: Vec<BoxedModelValidator>,
}

impl ValidatorCollection {
    /// Create empty collection
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a field validator
    pub fn add_field_validator(&mut self, validator: impl FieldValidator + 'static) {
        let field_name = validator.field_name().to_string();
        self.field_validators
            .entry(field_name)
            .or_default()
            .push(Arc::new(validator));
    }

    /// Add a model validator
    pub fn add_model_validator(&mut self, validator: impl ModelValidator + 'static) {
        self.model_validators.push(Arc::new(validator));
    }

    /// Get field validators for a specific field
    pub fn get_field_validators(&self, field_name: &str) -> Option<&Vec<BoxedFieldValidator>> {
        self.field_validators.get(field_name)
    }

    /// Get all model validators
    pub fn model_validators(&self) -> &[BoxedModelValidator] {
        &self.model_validators
    }

    /// Check if collection is empty
    pub fn is_empty(&self) -> bool {
        self.field_validators.is_empty() && self.model_validators.is_empty()
    }

    /// Run field validators for a specific field
    pub fn run_field_validators(
        &self,
        field_name: &str,
        value: &Value,
        context: &ValidatorContext,
        mode: ValidatorMode,
    ) -> ValidationResult<Value> {
        let Some(validators) = self.field_validators.get(field_name) else {
            return Ok(value.clone());
        };

        let mut current_value = value.clone();
        let mut errors = ValidationErrors::new();

        for validator in validators {
            if validator.mode() != mode {
                continue;
            }

            match validator.validate(&current_value, context) {
                Ok(new_value) => current_value = new_value,
                Err(e) => errors.merge(e),
            }
        }

        if errors.is_empty() {
            Ok(current_value)
        } else {
            Err(errors)
        }
    }

    /// Run model validators
    pub fn run_model_validators(
        &self,
        value: &Value,
        context: &ValidatorContext,
        mode: ValidatorMode,
    ) -> ValidationResult<Value> {
        let mut current_value = value.clone();
        let mut errors = ValidationErrors::new();

        for validator in &self.model_validators {
            if validator.mode() != mode {
                continue;
            }

            match validator.validate(&current_value, context) {
                Ok(new_value) => current_value = new_value,
                Err(e) => errors.merge(e),
            }
        }

        if errors.is_empty() {
            Ok(current_value)
        } else {
            Err(errors)
        }
    }
}

impl std::fmt::Debug for ValidatorCollection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ValidatorCollection")
            .field(
                "field_validators",
                &self.field_validators.keys().collect::<Vec<_>>(),
            )
            .field("model_validators_count", &self.model_validators.len())
            .finish()
    }
}

// ============================================================================
// Helper Functions for Creating Validation Errors
// ============================================================================

/// Create a validation error for a field
pub fn field_error(field: impl Into<String>, message: impl Into<String>) -> ValidationErrors {
    let mut errors = ValidationErrors::new();
    errors.add(ValidationError::new(
        String::new(),
        field.into(),
        message.into(),
        ErrorType::ValueError,
    ));
    errors
}

/// Create a validation error with custom error type
pub fn custom_error(
    field: impl Into<String>,
    message: impl Into<String>,
    error_type: ErrorType,
) -> ValidationErrors {
    let mut errors = ValidationErrors::new();
    errors.add(ValidationError::new(
        String::new(),
        field.into(),
        message.into(),
        error_type,
    ));
    errors
}

// ============================================================================
// Function-based Validators (for ergonomic API)
// ============================================================================

/// Create a field validator from a function
pub struct FnFieldValidator<F>
where
    F: Fn(&Value, &ValidatorContext) -> ValidationResult<Value> + Send + Sync,
{
    field_name: String,
    mode: ValidatorMode,
    validate_fn: F,
}

impl<F> FnFieldValidator<F>
where
    F: Fn(&Value, &ValidatorContext) -> ValidationResult<Value> + Send + Sync,
{
    /// Create a new function-based field validator
    pub fn new(field_name: impl Into<String>, validate_fn: F) -> Self {
        Self {
            field_name: field_name.into(),
            mode: ValidatorMode::After,
            validate_fn,
        }
    }

    /// Set validator mode
    pub fn mode(mut self, mode: ValidatorMode) -> Self {
        self.mode = mode;
        self
    }
}

impl<F> FieldValidator for FnFieldValidator<F>
where
    F: Fn(&Value, &ValidatorContext) -> ValidationResult<Value> + Send + Sync,
{
    fn field_name(&self) -> &str {
        &self.field_name
    }

    fn mode(&self) -> ValidatorMode {
        self.mode
    }

    fn validate(&self, value: &Value, context: &ValidatorContext) -> ValidationResult<Value> {
        (self.validate_fn)(value, context)
    }
}

/// Create a model validator from a function
pub struct FnModelValidator<F>
where
    F: Fn(&Value, &ValidatorContext) -> ValidationResult<Value> + Send + Sync,
{
    mode: ValidatorMode,
    validate_fn: F,
}

impl<F> FnModelValidator<F>
where
    F: Fn(&Value, &ValidatorContext) -> ValidationResult<Value> + Send + Sync,
{
    /// Create a new function-based model validator
    pub fn new(validate_fn: F) -> Self {
        Self {
            mode: ValidatorMode::After,
            validate_fn,
        }
    }

    /// Set validator mode
    pub fn mode(mut self, mode: ValidatorMode) -> Self {
        self.mode = mode;
        self
    }
}

impl<F> ModelValidator for FnModelValidator<F>
where
    F: Fn(&Value, &ValidatorContext) -> ValidationResult<Value> + Send + Sync,
{
    fn mode(&self) -> ValidatorMode {
        self.mode
    }

    fn validate(&self, value: &Value, context: &ValidatorContext) -> ValidationResult<Value> {
        (self.validate_fn)(value, context)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validator_context() {
        let mut ctx = ValidatorContext::new();
        ctx.set_field("name", Value::String("test".to_string()));
        ctx.set_field("age", Value::Int(25));

        assert_eq!(
            ctx.get_field("name"),
            Some(&Value::String("test".to_string()))
        );
        assert_eq!(ctx.get_field("age"), Some(&Value::Int(25)));
        assert_eq!(ctx.get_field("missing"), None);
    }

    #[test]
    fn test_fn_field_validator() {
        let validator = FnFieldValidator::new("age", |value, _ctx| {
            if let Value::Int(age) = value {
                if *age < 0 {
                    return Err(field_error("age", "Age must be non-negative"));
                }
            }
            Ok(value.clone())
        });

        let ctx = ValidatorContext::new();

        // Valid age
        let result = validator.validate(&Value::Int(25), &ctx);
        assert!(result.is_ok());

        // Invalid age
        let result = validator.validate(&Value::Int(-5), &ctx);
        assert!(result.is_err());
    }

    #[test]
    fn test_validator_collection() {
        let mut collection = ValidatorCollection::new();

        // Add field validator
        collection.add_field_validator(FnFieldValidator::new("name", |value, _ctx| {
            if let Value::String(s) = value {
                if s.is_empty() {
                    return Err(field_error("name", "Name cannot be empty"));
                }
            }
            Ok(value.clone())
        }));

        // Add model validator
        collection.add_model_validator(FnModelValidator::new(|value, _ctx| {
            // Just pass through for this test
            Ok(value.clone())
        }));

        assert!(!collection.is_empty());

        let ctx = ValidatorContext::new();

        // Valid name
        let result = collection.run_field_validators(
            "name",
            &Value::String("John".to_string()),
            &ctx,
            ValidatorMode::After,
        );
        assert!(result.is_ok());

        // Invalid name
        let result = collection.run_field_validators(
            "name",
            &Value::String("".to_string()),
            &ctx,
            ValidatorMode::After,
        );
        assert!(result.is_err());
    }
}
