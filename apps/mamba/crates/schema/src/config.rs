//! Configuration options for validation behavior
//!
//! Similar to Pydantic's ConfigDict for controlling validation behavior.

use crate::coercion::CoercionMode;
use crate::strict::StrictMode;

// ============================================================================
// Extra Field Handling
// ============================================================================

/// How to handle extra fields not defined in the schema
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExtraFields {
    /// Ignore extra fields (default)
    #[default]
    Ignore,
    /// Allow extra fields and include them in output
    Allow,
    /// Forbid extra fields (validation error)
    Forbid,
}

// ============================================================================
// Validation Config
// ============================================================================

/// Configuration options for validation behavior
#[derive(Debug, Clone, Default)]
pub struct ValidationConfig {
    /// How to handle extra fields
    pub extra: ExtraFields,

    /// Strip whitespace from strings
    pub str_strip_whitespace: bool,

    /// Convert strings to lowercase
    pub str_to_lower: bool,

    /// Convert strings to uppercase
    pub str_to_upper: bool,

    /// Validate values on assignment (for mutable types)
    pub validate_assignment: bool,

    /// Allow population by field name or alias
    pub populate_by_name: bool,

    /// Use enum values instead of names
    pub use_enum_values: bool,

    /// Strict mode configuration
    pub strict: StrictMode,

    /// Type coercion mode (Strict or Lax)
    pub coercion_mode: CoercionMode,

    /// Coerce numeric strings to numbers (deprecated, use coercion_mode)
    pub coerce_numbers_to_str: bool,

    /// Validate default values
    pub validate_default: bool,

    /// Revalidate instances on validation
    pub revalidate_instances: RevalidateInstances,

    /// Arbitrary types allowed
    pub arbitrary_types_allowed: bool,

    /// Whether to serialize by alias
    pub serialize_by_alias: bool,
}

/// When to revalidate model instances
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RevalidateInstances {
    /// Never revalidate
    #[default]
    Never,
    /// Always revalidate
    Always,
    /// Revalidate subclasses only
    Subclasses,
}

impl ValidationConfig {
    /// Create a new validation config with defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Set extra field handling
    pub fn extra(mut self, extra: ExtraFields) -> Self {
        self.extra = extra;
        self
    }

    /// Forbid extra fields
    pub fn forbid_extra(mut self) -> Self {
        self.extra = ExtraFields::Forbid;
        self
    }

    /// Allow extra fields
    pub fn allow_extra(mut self) -> Self {
        self.extra = ExtraFields::Allow;
        self
    }

    /// Enable string whitespace stripping
    pub fn strip_whitespace(mut self, strip: bool) -> Self {
        self.str_strip_whitespace = strip;
        self
    }

    /// Enable lowercase conversion
    pub fn to_lower(mut self, lower: bool) -> Self {
        self.str_to_lower = lower;
        self
    }

    /// Enable uppercase conversion
    pub fn to_upper(mut self, upper: bool) -> Self {
        self.str_to_upper = upper;
        self
    }

    /// Enable assignment validation
    pub fn validate_assignment(mut self, validate: bool) -> Self {
        self.validate_assignment = validate;
        self
    }

    /// Allow population by name
    pub fn populate_by_name(mut self, allow: bool) -> Self {
        self.populate_by_name = allow;
        self
    }

    /// Use enum values
    pub fn use_enum_values(mut self, use_values: bool) -> Self {
        self.use_enum_values = use_values;
        self
    }

    /// Set strict mode
    pub fn strict(mut self, strict: StrictMode) -> Self {
        self.strict = strict;
        self
    }

    /// Enable strict mode (all strict)
    pub fn strict_mode(mut self) -> Self {
        self.strict = StrictMode::strict();
        self
    }

    /// Enable coercion of numbers to strings (deprecated)
    pub fn coerce_numbers(mut self, coerce: bool) -> Self {
        self.coerce_numbers_to_str = coerce;
        self
    }

    /// Set coercion mode
    pub fn coercion_mode(mut self, mode: CoercionMode) -> Self {
        self.coercion_mode = mode;
        self
    }

    /// Enable lax coercion mode
    pub fn lax_mode(mut self) -> Self {
        self.coercion_mode = CoercionMode::Lax;
        self
    }

    /// Validate default values
    pub fn validate_default(mut self, validate: bool) -> Self {
        self.validate_default = validate;
        self
    }

    /// Set revalidation mode
    pub fn revalidate(mut self, mode: RevalidateInstances) -> Self {
        self.revalidate_instances = mode;
        self
    }

    /// Allow arbitrary types
    pub fn arbitrary_types(mut self, allow: bool) -> Self {
        self.arbitrary_types_allowed = allow;
        self
    }

    /// Serialize using aliases
    pub fn serialize_by_alias(mut self, by_alias: bool) -> Self {
        self.serialize_by_alias = by_alias;
        self
    }

    /// Process a string value according to config
    pub fn process_string(&self, s: &str) -> String {
        let mut result = s.to_string();

        if self.str_strip_whitespace {
            result = result.trim().to_string();
        }

        if self.str_to_lower {
            result = result.to_lowercase();
        } else if self.str_to_upper {
            result = result.to_uppercase();
        }

        result
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ValidationConfig::default();
        assert_eq!(config.extra, ExtraFields::Ignore);
        assert!(!config.str_strip_whitespace);
        assert!(!config.validate_assignment);
    }

    #[test]
    fn test_config_builder() {
        let config = ValidationConfig::new()
            .forbid_extra()
            .strip_whitespace(true)
            .validate_assignment(true)
            .strict_mode();

        assert_eq!(config.extra, ExtraFields::Forbid);
        assert!(config.str_strip_whitespace);
        assert!(config.validate_assignment);
        assert!(config.strict.strict_types);
    }

    #[test]
    fn test_process_string() {
        let config = ValidationConfig::new()
            .strip_whitespace(true)
            .to_lower(true);

        assert_eq!(config.process_string("  HELLO  "), "hello");
    }

    #[test]
    fn test_process_string_upper() {
        let config = ValidationConfig::new()
            .strip_whitespace(true)
            .to_upper(true);

        assert_eq!(config.process_string("  hello  "), "HELLO");
    }

    #[test]
    fn test_extra_fields() {
        assert_eq!(ExtraFields::default(), ExtraFields::Ignore);
        assert_ne!(ExtraFields::Forbid, ExtraFields::Allow);
    }

    #[test]
    fn test_revalidate_instances() {
        assert_eq!(RevalidateInstances::default(), RevalidateInstances::Never);
    }
}
