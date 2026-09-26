//! Discriminated unions for efficient union type resolution
//!
//! Provides O(1) type resolution using a discriminator field instead
//! of trying all variants sequentially.

use crate::errors::ValidationError;
use crate::types::{TypeDescriptor, Value};
use std::collections::HashMap;

// ============================================================================
// Discriminated Union
// ============================================================================

/// Discriminated union configuration
#[derive(Debug, Clone)]
pub struct DiscriminatedUnion {
    /// The field name used as discriminator
    pub discriminator: String,
    /// Mapping from discriminator value to variant index
    pub variants: HashMap<String, TypeDescriptor>,
}

impl DiscriminatedUnion {
    /// Create a new discriminated union
    pub fn new(discriminator: impl Into<String>) -> Self {
        Self {
            discriminator: discriminator.into(),
            variants: HashMap::new(),
        }
    }

    /// Add a variant with its discriminator value
    pub fn variant(mut self, value: impl Into<String>, type_desc: TypeDescriptor) -> Self {
        self.variants.insert(value.into(), type_desc);
        self
    }

    /// Get the variant for a discriminator value
    pub fn get_variant(&self, value: &str) -> Option<&TypeDescriptor> {
        self.variants.get(value)
    }

    /// Get all variant names
    pub fn variant_names(&self) -> Vec<&str> {
        self.variants.keys().map(|s| s.as_str()).collect()
    }

    /// Resolve the discriminator value from a value
    pub fn resolve_discriminator<'a>(&self, value: &'a Value) -> Result<&'a str, ValidationError> {
        match value {
            Value::Object(fields) => {
                // Look for discriminator field
                for (name, field_value) in fields {
                    if name == &self.discriminator {
                        return match field_value {
                            Value::String(s) => Ok(s.as_str()),
                            _ => Err(ValidationError::type_error(
                                "discriminator".to_string(),
                                self.discriminator.clone(),
                                format!("Expected string, got {}", field_value.type_name()),
                            )),
                        };
                    }
                }
                Err(ValidationError::missing_error(
                    "discriminator".to_string(),
                    self.discriminator.clone(),
                ))
            }
            _ => Err(ValidationError::type_error(
                "root".to_string(),
                "".to_string(),
                format!("Expected object, got {}", value.type_name()),
            )),
        }
    }

    /// Validate a value against the discriminated union
    pub fn validate(&self, value: &Value) -> Result<&TypeDescriptor, ValidationError> {
        // Get discriminator value
        let disc_value = self.resolve_discriminator(value)?;

        // Look up variant
        self.variants.get(disc_value).ok_or_else(|| {
            ValidationError::value_error(
                "discriminator".to_string(),
                self.discriminator.clone(),
                format!(
                    "Invalid discriminator value '{}'. Expected one of: {:?}",
                    disc_value,
                    self.variant_names()
                ),
            )
        })
    }
}

// ============================================================================
// Builder
// ============================================================================

/// Builder for discriminated unions
pub struct DiscriminatedUnionBuilder {
    discriminator: String,
    variants: Vec<(String, TypeDescriptor)>,
}

impl DiscriminatedUnionBuilder {
    /// Create a new builder
    pub fn new(discriminator: impl Into<String>) -> Self {
        Self {
            discriminator: discriminator.into(),
            variants: Vec::new(),
        }
    }

    /// Add a variant
    pub fn variant(mut self, value: impl Into<String>, type_desc: TypeDescriptor) -> Self {
        self.variants.push((value.into(), type_desc));
        self
    }

    /// Build the discriminated union
    pub fn build(self) -> DiscriminatedUnion {
        let mut union = DiscriminatedUnion::new(self.discriminator);
        for (value, type_desc) in self.variants {
            union.variants.insert(value, type_desc);
        }
        union
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constraints::{FieldDescriptor, NumericConstraints, StringConstraints};

    fn make_cat_type() -> TypeDescriptor {
        TypeDescriptor::Object {
            fields: vec![
                FieldDescriptor::new(
                    "pet_type",
                    TypeDescriptor::String(StringConstraints::default()),
                ),
                FieldDescriptor::new(
                    "meows",
                    TypeDescriptor::Int64(NumericConstraints::default()),
                ),
            ],
            additional: None,
        }
    }

    fn make_dog_type() -> TypeDescriptor {
        TypeDescriptor::Object {
            fields: vec![
                FieldDescriptor::new(
                    "pet_type",
                    TypeDescriptor::String(StringConstraints::default()),
                ),
                FieldDescriptor::new(
                    "barks",
                    TypeDescriptor::Float64(NumericConstraints::default()),
                ),
            ],
            additional: None,
        }
    }

    #[test]
    fn test_discriminated_union_builder() {
        let union = DiscriminatedUnionBuilder::new("pet_type")
            .variant("cat", make_cat_type())
            .variant("dog", make_dog_type())
            .build();

        assert_eq!(union.discriminator, "pet_type");
        assert_eq!(union.variants.len(), 2);
        assert!(union.get_variant("cat").is_some());
        assert!(union.get_variant("dog").is_some());
        assert!(union.get_variant("fish").is_none());
    }

    #[test]
    fn test_resolve_discriminator() {
        let union = DiscriminatedUnion::new("type")
            .variant("a", TypeDescriptor::String(Default::default()));

        let value = Value::Object(vec![("type".to_string(), Value::String("a".to_string()))]);

        let disc = union.resolve_discriminator(&value).unwrap();
        assert_eq!(disc, "a");
    }

    #[test]
    fn test_resolve_missing_discriminator() {
        let union = DiscriminatedUnion::new("type")
            .variant("a", TypeDescriptor::String(Default::default()));

        let value = Value::Object(vec![(
            "other".to_string(),
            Value::String("value".to_string()),
        )]);

        let result = union.resolve_discriminator(&value);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_discriminated_union() {
        let union = DiscriminatedUnionBuilder::new("pet_type")
            .variant("cat", make_cat_type())
            .variant("dog", make_dog_type())
            .build();

        let cat_value = Value::Object(vec![
            ("pet_type".to_string(), Value::String("cat".to_string())),
            ("meows".to_string(), Value::Int(10)),
        ]);

        let dog_value = Value::Object(vec![
            ("pet_type".to_string(), Value::String("dog".to_string())),
            ("barks".to_string(), Value::Float(3.5)),
        ]);

        // Should resolve to correct variant
        assert!(union.validate(&cat_value).is_ok());
        assert!(union.validate(&dog_value).is_ok());

        // Invalid discriminator
        let fish_value = Value::Object(vec![(
            "pet_type".to_string(),
            Value::String("fish".to_string()),
        )]);
        assert!(union.validate(&fish_value).is_err());
    }

    #[test]
    fn test_variant_names() {
        let union = DiscriminatedUnion::new("kind")
            .variant("type_a", TypeDescriptor::Bool)
            .variant("type_b", TypeDescriptor::Bool)
            .variant("type_c", TypeDescriptor::Bool);

        let names = union.variant_names();
        assert_eq!(names.len(), 3);
        assert!(names.contains(&"type_a"));
        assert!(names.contains(&"type_b"));
        assert!(names.contains(&"type_c"));
    }
}
