//! Custom field serializers for output formatting
//!
//! This module provides a trait-based system for custom serializers,
//! similar to Pydantic's `@field_serializer`.
//!
//! # Example (Rust)
//!
//! ```rust,ignore
//! use schema::serializers::{FieldSerializer, SerializerContext};
//! use schema::Value;
//!
//! struct DateFormatter;
//!
//! impl FieldSerializer for DateFormatter {
//!     fn field_name(&self) -> &str { "timestamp" }
//!
//!     fn serialize(&self, value: &Value, _ctx: &SerializerContext) -> Value {
//!         if let Value::String(dt) = value {
//!             // Custom date formatting logic
//!             Value::String(format!("formatted: {}", dt))
//!         } else {
//!             value.clone()
//!         }
//!     }
//! }
//! ```

use crate::types::Value;
use std::collections::HashMap;
use std::sync::Arc;

// ============================================================================
// Serializer Mode
// ============================================================================

/// When to run the serializer in the serialization pipeline
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SerializerMode {
    /// Run on all serialization (default)
    #[default]
    Always,
    /// Only when serializing to JSON
    Json,
    /// Only when serializing to Python dict
    Python,
    /// Custom mode identifier
    Custom(&'static str),
}

// ============================================================================
// Serializer Context
// ============================================================================

/// Context passed to serializers during serialization
#[derive(Debug, Clone, Default)]
pub struct SerializerContext {
    /// Output format being produced
    format: String,
    /// Include null/None values in output
    include_none: bool,
    /// Exclude fields by name
    exclude_fields: Vec<String>,
    /// Include only these fields (if non-empty)
    include_fields: Vec<String>,
    /// Custom metadata
    metadata: HashMap<String, String>,
}

impl SerializerContext {
    /// Create a new empty context
    pub fn new() -> Self {
        Self::default()
    }

    /// Create context for JSON output
    pub fn json() -> Self {
        Self {
            format: "json".to_string(),
            ..Default::default()
        }
    }

    /// Create context for Python dict output
    pub fn python() -> Self {
        Self {
            format: "python".to_string(),
            ..Default::default()
        }
    }

    /// Get the output format
    pub fn format(&self) -> &str {
        &self.format
    }

    /// Set whether to include None values
    pub fn with_include_none(mut self, include: bool) -> Self {
        self.include_none = include;
        self
    }

    /// Whether to include None values
    pub fn include_none(&self) -> bool {
        self.include_none
    }

    /// Set fields to exclude
    pub fn with_exclude(mut self, fields: Vec<String>) -> Self {
        self.exclude_fields = fields;
        self
    }

    /// Check if a field should be excluded
    pub fn should_exclude(&self, field: &str) -> bool {
        if !self.exclude_fields.is_empty() && self.exclude_fields.contains(&field.to_string()) {
            return true;
        }
        if !self.include_fields.is_empty() && !self.include_fields.contains(&field.to_string()) {
            return true;
        }
        false
    }

    /// Set fields to include
    pub fn with_include(mut self, fields: Vec<String>) -> Self {
        self.include_fields = fields;
        self
    }

    /// Set metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Get metadata
    pub fn get_metadata(&self, key: &str) -> Option<&str> {
        self.metadata.get(key).map(|s| s.as_str())
    }
}

// ============================================================================
// Field Serializer Trait
// ============================================================================

/// Trait for field-level serializers (like Pydantic's @field_serializer)
///
/// Field serializers transform a field's value during serialization.
pub trait FieldSerializer: Send + Sync {
    /// Name of the field this serializer applies to
    fn field_name(&self) -> &str;

    /// When to run this serializer
    fn mode(&self) -> SerializerMode {
        SerializerMode::Always
    }

    /// Serialize the field value
    ///
    /// # Arguments
    /// * `value` - The field value to serialize
    /// * `context` - Serialization context
    ///
    /// # Returns
    /// The serialized value
    fn serialize(&self, value: &Value, context: &SerializerContext) -> Value;
}

// ============================================================================
// Model Serializer Trait
// ============================================================================

/// Trait for model-level serializers
///
/// Model serializers can transform the entire object during serialization.
pub trait ModelSerializer: Send + Sync {
    /// When to run this serializer
    fn mode(&self) -> SerializerMode {
        SerializerMode::Always
    }

    /// Serialize the model
    ///
    /// # Arguments
    /// * `value` - The model (Object) value
    /// * `context` - Serialization context
    ///
    /// # Returns
    /// The serialized model
    fn serialize(&self, value: &Value, context: &SerializerContext) -> Value;
}

// ============================================================================
// Boxed Types for Dynamic Dispatch
// ============================================================================

/// Type alias for boxed field serializer
pub type BoxedFieldSerializer = Arc<dyn FieldSerializer>;

/// Type alias for boxed model serializer
pub type BoxedModelSerializer = Arc<dyn ModelSerializer>;

// ============================================================================
// Serializer Collection
// ============================================================================

/// Collection of serializers for a model/type
#[derive(Default, Clone)]
pub struct SerializerCollection {
    /// Field serializers grouped by field name
    field_serializers: HashMap<String, Vec<BoxedFieldSerializer>>,
    /// Model serializers
    model_serializers: Vec<BoxedModelSerializer>,
}

impl SerializerCollection {
    /// Create empty collection
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a field serializer
    pub fn add_field_serializer(&mut self, serializer: impl FieldSerializer + 'static) {
        let field_name = serializer.field_name().to_string();
        self.field_serializers
            .entry(field_name)
            .or_default()
            .push(Arc::new(serializer));
    }

    /// Add a model serializer
    pub fn add_model_serializer(&mut self, serializer: impl ModelSerializer + 'static) {
        self.model_serializers.push(Arc::new(serializer));
    }

    /// Get field serializers for a specific field
    pub fn get_field_serializers(&self, field_name: &str) -> Option<&Vec<BoxedFieldSerializer>> {
        self.field_serializers.get(field_name)
    }

    /// Get all model serializers
    pub fn model_serializers(&self) -> &[BoxedModelSerializer] {
        &self.model_serializers
    }

    /// Check if collection is empty
    pub fn is_empty(&self) -> bool {
        self.field_serializers.is_empty() && self.model_serializers.is_empty()
    }

    /// Serialize a field value
    pub fn serialize_field(
        &self,
        field_name: &str,
        value: &Value,
        context: &SerializerContext,
    ) -> Value {
        let Some(serializers) = self.field_serializers.get(field_name) else {
            return value.clone();
        };

        let mut current_value = value.clone();
        for serializer in serializers {
            // Check mode
            match serializer.mode() {
                SerializerMode::Always => {}
                SerializerMode::Json if context.format() == "json" => {}
                SerializerMode::Python if context.format() == "python" => {}
                SerializerMode::Custom(mode) if context.format() == mode => {}
                _ => continue,
            }
            current_value = serializer.serialize(&current_value, context);
        }
        current_value
    }

    /// Serialize an entire model
    pub fn serialize_model(&self, value: &Value, context: &SerializerContext) -> Value {
        let mut current_value = value.clone();

        // Run model serializers
        for serializer in &self.model_serializers {
            match serializer.mode() {
                SerializerMode::Always => {}
                SerializerMode::Json if context.format() == "json" => {}
                SerializerMode::Python if context.format() == "python" => {}
                SerializerMode::Custom(mode) if context.format() == mode => {}
                _ => continue,
            }
            current_value = serializer.serialize(&current_value, context);
        }

        // If it's an object, serialize each field
        if let Value::Object(fields) = &current_value {
            let mut new_fields = Vec::new();
            for (name, field_value) in fields {
                if context.should_exclude(name) {
                    continue;
                }
                if !context.include_none() && field_value.is_null() {
                    continue;
                }
                let serialized = self.serialize_field(name, field_value, context);
                new_fields.push((name.clone(), serialized));
            }
            current_value = Value::Object(new_fields);
        }

        current_value
    }
}

impl std::fmt::Debug for SerializerCollection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SerializerCollection")
            .field(
                "field_serializers",
                &self.field_serializers.keys().collect::<Vec<_>>(),
            )
            .field("model_serializers_count", &self.model_serializers.len())
            .finish()
    }
}

// ============================================================================
// Function-based Serializers
// ============================================================================

/// Create a field serializer from a function
pub struct FnFieldSerializer<F>
where
    F: Fn(&Value, &SerializerContext) -> Value + Send + Sync,
{
    field_name: String,
    mode: SerializerMode,
    serialize_fn: F,
}

impl<F> FnFieldSerializer<F>
where
    F: Fn(&Value, &SerializerContext) -> Value + Send + Sync,
{
    /// Create a new function-based field serializer
    pub fn new(field_name: impl Into<String>, serialize_fn: F) -> Self {
        Self {
            field_name: field_name.into(),
            mode: SerializerMode::Always,
            serialize_fn,
        }
    }

    /// Set serializer mode
    pub fn mode(mut self, mode: SerializerMode) -> Self {
        self.mode = mode;
        self
    }
}

impl<F> FieldSerializer for FnFieldSerializer<F>
where
    F: Fn(&Value, &SerializerContext) -> Value + Send + Sync,
{
    fn field_name(&self) -> &str {
        &self.field_name
    }

    fn mode(&self) -> SerializerMode {
        self.mode
    }

    fn serialize(&self, value: &Value, context: &SerializerContext) -> Value {
        (self.serialize_fn)(value, context)
    }
}

/// Create a model serializer from a function
pub struct FnModelSerializer<F>
where
    F: Fn(&Value, &SerializerContext) -> Value + Send + Sync,
{
    mode: SerializerMode,
    serialize_fn: F,
}

impl<F> FnModelSerializer<F>
where
    F: Fn(&Value, &SerializerContext) -> Value + Send + Sync,
{
    /// Create a new function-based model serializer
    pub fn new(serialize_fn: F) -> Self {
        Self {
            mode: SerializerMode::Always,
            serialize_fn,
        }
    }

    /// Set serializer mode
    pub fn mode(mut self, mode: SerializerMode) -> Self {
        self.mode = mode;
        self
    }
}

impl<F> ModelSerializer for FnModelSerializer<F>
where
    F: Fn(&Value, &SerializerContext) -> Value + Send + Sync,
{
    fn mode(&self) -> SerializerMode {
        self.mode
    }

    fn serialize(&self, value: &Value, context: &SerializerContext) -> Value {
        (self.serialize_fn)(value, context)
    }
}

// ============================================================================
// Common Serializers
// ============================================================================

/// A serializer that masks sensitive data
pub struct MaskSerializer {
    field_name: String,
    mask_char: char,
    visible_chars: usize,
}

impl MaskSerializer {
    /// Create a new mask serializer
    ///
    /// # Arguments
    /// * `field_name` - Field to mask
    /// * `mask_char` - Character to use for masking (default: '*')
    /// * `visible_chars` - Number of characters to leave visible at the end
    pub fn new(field_name: impl Into<String>, mask_char: char, visible_chars: usize) -> Self {
        Self {
            field_name: field_name.into(),
            mask_char,
            visible_chars,
        }
    }
}

impl FieldSerializer for MaskSerializer {
    fn field_name(&self) -> &str {
        &self.field_name
    }

    fn serialize(&self, value: &Value, _context: &SerializerContext) -> Value {
        if let Value::String(s) = value {
            if s.len() <= self.visible_chars {
                return Value::String(self.mask_char.to_string().repeat(s.len()));
            }
            let visible_part = &s[s.len() - self.visible_chars..];
            let masked_part = self
                .mask_char
                .to_string()
                .repeat(s.len() - self.visible_chars);
            Value::String(format!("{}{}", masked_part, visible_part))
        } else {
            value.clone()
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
    fn test_serializer_context() {
        let ctx = SerializerContext::json()
            .with_include_none(false)
            .with_exclude(vec!["password".to_string()]);

        assert_eq!(ctx.format(), "json");
        assert!(!ctx.include_none());
        assert!(ctx.should_exclude("password"));
        assert!(!ctx.should_exclude("username"));
    }

    #[test]
    fn test_fn_field_serializer() {
        let serializer = FnFieldSerializer::new("name", |value, _ctx| {
            if let Value::String(s) = value {
                Value::String(s.to_uppercase())
            } else {
                value.clone()
            }
        });

        let ctx = SerializerContext::new();
        let result = serializer.serialize(&Value::String("john".to_string()), &ctx);
        assert_eq!(result, Value::String("JOHN".to_string()));
    }

    #[test]
    fn test_mask_serializer() {
        let serializer = MaskSerializer::new("card_number", '*', 4);
        let ctx = SerializerContext::new();

        let result = serializer.serialize(&Value::String("1234567890123456".to_string()), &ctx);
        assert_eq!(result, Value::String("************3456".to_string()));
    }

    #[test]
    fn test_serializer_collection() {
        let mut collection = SerializerCollection::new();

        // Add field serializer
        collection.add_field_serializer(FnFieldSerializer::new("email", |value, _ctx| {
            if let Value::String(s) = value {
                Value::String(s.to_lowercase())
            } else {
                value.clone()
            }
        }));

        let ctx = SerializerContext::new();

        // Test field serialization
        let result = collection.serialize_field(
            "email",
            &Value::String("JOHN@EXAMPLE.COM".to_string()),
            &ctx,
        );
        assert_eq!(result, Value::String("john@example.com".to_string()));

        // Test non-serialized field
        let result = collection.serialize_field("name", &Value::String("John".to_string()), &ctx);
        assert_eq!(result, Value::String("John".to_string()));
    }

    #[test]
    fn test_serialize_model() {
        let mut collection = SerializerCollection::new();

        collection.add_field_serializer(MaskSerializer::new("password", '*', 0));

        let ctx = SerializerContext::new();
        let model = Value::Object(vec![
            ("username".to_string(), Value::String("john".to_string())),
            (
                "password".to_string(),
                Value::String("secret123".to_string()),
            ),
        ]);

        let result = collection.serialize_model(&model, &ctx);
        let Value::Object(fields) = result else {
            unreachable!("Expected object");
        };
        let password = fields.iter().find(|(k, _)| k == "password").unwrap();
        assert_eq!(password.1, Value::String("*********".to_string()));
    }
}
