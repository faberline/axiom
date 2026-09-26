//! Validation constraints for different types
//!
//! This module defines constraints for validating strings, numbers, lists, and object fields.

use crate::types::{TypeDescriptor, Value};

// ============================================================================
// String Constraints
// ============================================================================

/// Constraints for string validation
#[derive(Debug, Clone, Default)]
pub struct StringConstraints {
    /// Minimum length (in characters, not bytes)
    pub min_length: Option<usize>,
    /// Maximum length (in characters, not bytes)
    pub max_length: Option<usize>,
    /// Regex pattern (compiled at validation time)
    pub pattern: Option<String>,
    /// Predefined format validator
    pub format: Option<StringFormat>,
}

/// Predefined string format validators
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StringFormat {
    /// Email address format
    Email,
    /// URL format (http/https)
    Url,
    /// UUID format (v4)
    Uuid,
    /// ISO 8601 DateTime format
    DateTime,
    /// Date format (YYYY-MM-DD)
    Date,
    /// Time format (HH:MM:SS)
    Time,

    // ========================================================================
    // New Formats (Phase 1 Enhancement)
    // ========================================================================
    /// IPv4 address format
    Ipv4,
    /// IPv6 address format
    Ipv6,
    /// Hostname format (RFC 1123)
    Hostname,
    /// Fully Qualified Domain Name
    Fqdn,
    /// E.164 phone number format
    Phone,
    /// Base64 encoded string
    Base64,
    /// URL-friendly slug
    Slug,
    /// Valid JSON string
    Json,
}

// ============================================================================
// Numeric Constraints
// ============================================================================

/// Constraints for numeric validation (generic over i64 and f64)
#[derive(Debug, Clone, Default)]
pub struct NumericConstraints<T> {
    /// Minimum value (inclusive)
    pub minimum: Option<T>,
    /// Maximum value (inclusive)
    pub maximum: Option<T>,
    /// Minimum value (exclusive)
    pub exclusive_minimum: Option<T>,
    /// Maximum value (exclusive)
    pub exclusive_maximum: Option<T>,
    /// Value must be a multiple of this number
    pub multiple_of: Option<T>,
}

// ============================================================================
// List Constraints
// ============================================================================

/// Constraints for list/array validation
#[derive(Debug, Clone, Default)]
pub struct ListConstraints {
    /// Minimum number of items
    pub min_items: Option<usize>,
    /// Maximum number of items
    pub max_items: Option<usize>,
    /// Whether all items must be unique
    pub unique_items: bool,
}

// ============================================================================
// Field Descriptor (for Object validation)
// ============================================================================

/// Field descriptor for object/dictionary validation
#[derive(Debug, Clone)]
pub struct FieldDescriptor {
    /// Field name
    pub name: String,
    /// Type descriptor for this field
    pub type_desc: TypeDescriptor,
    /// Whether this field is required
    pub required: bool,
    /// Default value if field is missing
    pub default: Option<Value>,
    /// Optional description for documentation
    pub description: Option<String>,
    /// Optional JSON Schema/OpenAPI title
    pub title: Option<String>,
    /// Optional JSON Schema/OpenAPI examples
    pub examples: Vec<Value>,
    /// Alias for both validation and serialization
    /// When set, the field can be accessed by either name or alias
    pub alias: Option<String>,
    /// Alias for validation only (input)
    /// When set, input can use this alias but output uses original name
    pub validation_alias: Option<String>,
    /// Alias for serialization only (output)
    /// When set, output uses this alias but input uses original name
    pub serialization_alias: Option<String>,
    /// Private field - excluded from serialization output
    /// Similar to Pydantic's PrivateAttr
    pub private: bool,
    /// Init-only field - can only be set during initialization
    pub init_only: bool,
    /// Mark field as deprecated in JSON Schema/OpenAPI
    pub deprecated: bool,
    /// Mark field as read-only in JSON Schema/OpenAPI
    pub read_only: bool,
    /// Mark field as write-only in JSON Schema/OpenAPI
    pub write_only: bool,
}

impl FieldDescriptor {
    /// Create a new field descriptor
    pub fn new(name: impl Into<String>, type_desc: TypeDescriptor) -> Self {
        Self {
            name: name.into(),
            type_desc,
            required: true,
            default: None,
            description: None,
            title: None,
            examples: Vec::new(),
            alias: None,
            validation_alias: None,
            serialization_alias: None,
            private: false,
            init_only: false,
            deprecated: false,
            read_only: false,
            write_only: false,
        }
    }

    /// Set field as optional
    pub fn optional(mut self) -> Self {
        self.required = false;
        self
    }

    /// Set default value
    pub fn default_value(mut self, value: Value) -> Self {
        self.default = Some(value);
        self.required = false;
        self
    }

    /// Set description
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Set JSON Schema/OpenAPI title
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set JSON Schema/OpenAPI examples
    pub fn examples(mut self, examples: Vec<Value>) -> Self {
        self.examples = examples;
        self
    }

    /// Set alias for both validation and serialization
    pub fn alias(mut self, alias: impl Into<String>) -> Self {
        self.alias = Some(alias.into());
        self
    }

    /// Set alias for validation only
    pub fn validation_alias(mut self, alias: impl Into<String>) -> Self {
        self.validation_alias = Some(alias.into());
        self
    }

    /// Set alias for serialization only
    pub fn serialization_alias(mut self, alias: impl Into<String>) -> Self {
        self.serialization_alias = Some(alias.into());
        self
    }

    /// Mark field as private (excluded from serialization)
    pub fn private(mut self) -> Self {
        self.private = true;
        self
    }

    /// Mark field as init-only (can only be set during initialization)
    pub fn init_only(mut self) -> Self {
        self.init_only = true;
        self
    }

    /// Mark field as deprecated in JSON Schema/OpenAPI
    pub fn deprecated(mut self) -> Self {
        self.deprecated = true;
        self
    }

    /// Mark field as read-only in JSON Schema/OpenAPI
    pub fn read_only(mut self) -> Self {
        self.read_only = true;
        self
    }

    /// Mark field as write-only in JSON Schema/OpenAPI
    pub fn write_only(mut self) -> Self {
        self.write_only = true;
        self
    }

    /// Check if field should be included in serialization
    pub fn should_serialize(&self) -> bool {
        !self.private
    }

    /// Get the name to use for validation (input)
    pub fn validation_name(&self) -> &str {
        self.validation_alias
            .as_deref()
            .or(self.alias.as_deref())
            .unwrap_or(&self.name)
    }

    /// Get the name to use for serialization (output)
    pub fn serialization_name(&self) -> &str {
        self.serialization_alias
            .as_deref()
            .or(self.alias.as_deref())
            .unwrap_or(&self.name)
    }

    /// Get all names that can be used to access this field (for validation)
    pub fn all_validation_names(&self) -> Vec<&str> {
        let mut names = vec![self.name.as_str()];
        if let Some(ref alias) = self.alias {
            names.push(alias.as_str());
        }
        if let Some(ref alias) = self.validation_alias {
            names.push(alias.as_str());
        }
        names
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_constraints_default() {
        let constraints = StringConstraints::default();
        assert!(constraints.min_length.is_none());
        assert!(constraints.max_length.is_none());
        assert!(constraints.pattern.is_none());
        assert!(constraints.format.is_none());
    }

    #[test]
    fn test_numeric_constraints_default() {
        let constraints: NumericConstraints<i64> = NumericConstraints::default();
        assert!(constraints.minimum.is_none());
        assert!(constraints.maximum.is_none());
        assert!(constraints.exclusive_minimum.is_none());
        assert!(constraints.exclusive_maximum.is_none());
        assert!(constraints.multiple_of.is_none());
    }

    #[test]
    fn test_list_constraints_default() {
        let constraints = ListConstraints::default();
        assert!(constraints.min_items.is_none());
        assert!(constraints.max_items.is_none());
        assert!(!constraints.unique_items);
    }

    #[test]
    fn test_string_format_equality() {
        assert_eq!(StringFormat::Email, StringFormat::Email);
        assert_ne!(StringFormat::Email, StringFormat::Url);
    }

    #[test]
    fn test_field_descriptor_private() {
        use crate::TypeDescriptor;

        let field =
            FieldDescriptor::new("password", TypeDescriptor::String(Default::default())).private();

        assert!(field.private);
        assert!(!field.should_serialize());

        let public_field = FieldDescriptor::new("name", TypeDescriptor::String(Default::default()));
        assert!(!public_field.private);
        assert!(public_field.should_serialize());
    }

    #[test]
    fn test_field_descriptor_init_only() {
        use crate::TypeDescriptor;

        let field =
            FieldDescriptor::new("id", TypeDescriptor::Int64(Default::default())).init_only();

        assert!(field.init_only);
    }
}
