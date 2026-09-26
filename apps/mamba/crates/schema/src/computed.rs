//! Computed field support for cclab-shield
//!
//! This module provides Pydantic-like computed fields for derived properties.
//!
//! # Example (Rust)
//!
//! ```rust,ignore
//! use schema::computed::{ComputedField, ComputedFieldCollection};
//! use schema::Value;
//!
//! struct FullNameComputed;
//!
//! impl ComputedField for FullNameComputed {
//!     fn field_name(&self) -> &str { "full_name" }
//!
//!     fn compute(&self, model: &Value) -> Value {
//!         if let Value::Object(fields) = model {
//!             let first = fields.iter()
//!                 .find(|(k, _)| k == "first_name")
//!                 .map(|(_, v)| v.as_string().unwrap_or(""));
//!             let last = fields.iter()
//!                 .find(|(k, _)| k == "last_name")
//!                 .map(|(_, v)| v.as_string().unwrap_or(""));
//!             Value::String(format!("{} {}", first.unwrap_or(""), last.unwrap_or("")))
//!         } else {
//!             Value::Null
//!         }
//!     }
//! }
//! ```

use crate::types::Value;
use std::collections::HashMap;
use std::sync::Arc;

// ============================================================================
// Computed Field Trait
// ============================================================================

/// Trait for computed fields (like Pydantic's @computed_field)
///
/// Computed fields are derived from other model fields and calculated
/// during serialization.
pub trait ComputedField: Send + Sync {
    /// Name of the computed field
    fn field_name(&self) -> &str;

    /// Compute the field value from the model
    ///
    /// # Arguments
    /// * `model` - The model value (typically an Object)
    ///
    /// # Returns
    /// The computed field value
    fn compute(&self, model: &Value) -> Value;

    /// Whether this field should be included in serialization
    ///
    /// Override to conditionally include/exclude the computed field.
    fn should_include(&self, _model: &Value) -> bool {
        true
    }

    /// Description for documentation/schema generation
    fn description(&self) -> Option<&str> {
        None
    }

    /// Return type annotation (for schema generation)
    fn return_type(&self) -> Option<&str> {
        None
    }
}

/// Boxed computed field for dynamic dispatch
pub type BoxedComputedField = Arc<dyn ComputedField>;

// ============================================================================
// Function-based Computed Field
// ============================================================================

/// Computed field created from a function
pub struct FnComputedField<F>
where
    F: Fn(&Value) -> Value + Send + Sync,
{
    field_name: String,
    compute_fn: F,
    description: Option<String>,
    return_type: Option<String>,
}

impl<F> FnComputedField<F>
where
    F: Fn(&Value) -> Value + Send + Sync,
{
    /// Create a new function-based computed field
    pub fn new(field_name: impl Into<String>, compute_fn: F) -> Self {
        Self {
            field_name: field_name.into(),
            compute_fn,
            description: None,
            return_type: None,
        }
    }

    /// Set description
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Set return type annotation
    pub fn return_type(mut self, rt: impl Into<String>) -> Self {
        self.return_type = Some(rt.into());
        self
    }
}

impl<F> ComputedField for FnComputedField<F>
where
    F: Fn(&Value) -> Value + Send + Sync,
{
    fn field_name(&self) -> &str {
        &self.field_name
    }

    fn compute(&self, model: &Value) -> Value {
        (self.compute_fn)(model)
    }

    fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    fn return_type(&self) -> Option<&str> {
        self.return_type.as_deref()
    }
}

// ============================================================================
// Computed Field Collection
// ============================================================================

/// Collection of computed fields for a model
#[derive(Default, Clone)]
pub struct ComputedFieldCollection {
    fields: HashMap<String, BoxedComputedField>,
    /// Order in which fields should be computed (for dependencies)
    field_order: Vec<String>,
}

impl ComputedFieldCollection {
    /// Create empty collection
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a computed field
    pub fn add(&mut self, field: impl ComputedField + 'static) {
        let name = field.field_name().to_string();
        self.fields.insert(name.clone(), Arc::new(field));
        self.field_order.push(name);
    }

    /// Add a computed field from a function
    pub fn add_fn<F>(&mut self, field_name: impl Into<String>, compute_fn: F)
    where
        F: Fn(&Value) -> Value + Send + Sync + 'static,
    {
        self.add(FnComputedField::new(field_name, compute_fn));
    }

    /// Get a computed field by name
    pub fn get(&self, field_name: &str) -> Option<&BoxedComputedField> {
        self.fields.get(field_name)
    }

    /// Check if collection is empty
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    /// Get number of computed fields
    pub fn len(&self) -> usize {
        self.fields.len()
    }

    /// Get field names in order
    pub fn field_names(&self) -> &[String] {
        &self.field_order
    }

    /// Apply computed fields to a model
    ///
    /// Returns a new model value with computed fields added.
    pub fn apply(&self, model: &Value) -> Value {
        if self.is_empty() {
            return model.clone();
        }

        // Only apply to objects
        let Value::Object(fields) = model else {
            return model.clone();
        };

        // Clone existing fields
        let mut new_fields = fields.clone();

        // Add computed fields
        for name in &self.field_order {
            if let Some(computed) = self.fields.get(name) {
                if computed.should_include(model) {
                    let value = computed.compute(model);
                    new_fields.push((name.clone(), value));
                }
            }
        }

        Value::Object(new_fields)
    }

    /// Apply computed fields in place (modifies the model)
    ///
    /// Returns the computed values as a separate map.
    pub fn compute_values(&self, model: &Value) -> HashMap<String, Value> {
        let mut computed = HashMap::new();

        for name in &self.field_order {
            if let Some(field) = self.fields.get(name) {
                if field.should_include(model) {
                    computed.insert(name.clone(), field.compute(model));
                }
            }
        }

        computed
    }
}

impl std::fmt::Debug for ComputedFieldCollection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ComputedFieldCollection")
            .field("fields", &self.field_order)
            .finish()
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Helper to extract a string field from a model
pub fn get_string_field(model: &Value, field_name: &str) -> Option<String> {
    if let Value::Object(fields) = model {
        fields
            .iter()
            .find(|(k, _)| k == field_name)
            .and_then(|(_, v)| {
                if let Value::String(s) = v {
                    Some(s.clone())
                } else {
                    None
                }
            })
    } else {
        None
    }
}

/// Helper to extract an integer field from a model
pub fn get_int_field(model: &Value, field_name: &str) -> Option<i64> {
    if let Value::Object(fields) = model {
        fields
            .iter()
            .find(|(k, _)| k == field_name)
            .and_then(|(_, v)| {
                if let Value::Int(i) = v {
                    Some(*i)
                } else {
                    None
                }
            })
    } else {
        None
    }
}

/// Helper to extract a float field from a model
pub fn get_float_field(model: &Value, field_name: &str) -> Option<f64> {
    if let Value::Object(fields) = model {
        fields
            .iter()
            .find(|(k, _)| k == field_name)
            .and_then(|(_, v)| match v {
                Value::Float(f) => Some(*f),
                Value::Int(i) => Some(*i as f64),
                _ => None,
            })
    } else {
        None
    }
}

/// Helper to extract a boolean field from a model
pub fn get_bool_field(model: &Value, field_name: &str) -> Option<bool> {
    if let Value::Object(fields) = model {
        fields
            .iter()
            .find(|(k, _)| k == field_name)
            .and_then(|(_, v)| {
                if let Value::Bool(b) = v {
                    Some(*b)
                } else {
                    None
                }
            })
    } else {
        None
    }
}

// ============================================================================
// Common Computed Fields
// ============================================================================

/// A computed field that concatenates string fields
pub struct ConcatComputed {
    field_name: String,
    source_fields: Vec<String>,
    separator: String,
}

impl ConcatComputed {
    /// Create a new concat computed field
    pub fn new(
        field_name: impl Into<String>,
        source_fields: Vec<String>,
        separator: impl Into<String>,
    ) -> Self {
        Self {
            field_name: field_name.into(),
            source_fields,
            separator: separator.into(),
        }
    }
}

impl ComputedField for ConcatComputed {
    fn field_name(&self) -> &str {
        &self.field_name
    }

    fn compute(&self, model: &Value) -> Value {
        let parts: Vec<String> = self
            .source_fields
            .iter()
            .filter_map(|f| get_string_field(model, f))
            .collect();

        Value::String(parts.join(&self.separator))
    }

    fn return_type(&self) -> Option<&str> {
        Some("str")
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_person_model() -> Value {
        Value::Object(vec![
            ("first_name".to_string(), Value::String("John".to_string())),
            ("last_name".to_string(), Value::String("Doe".to_string())),
            ("age".to_string(), Value::Int(30)),
        ])
    }

    #[test]
    fn test_fn_computed_field() {
        let computed = FnComputedField::new("full_name", |model| {
            let first = get_string_field(model, "first_name").unwrap_or_default();
            let last = get_string_field(model, "last_name").unwrap_or_default();
            Value::String(format!("{} {}", first, last))
        });

        let model = create_person_model();
        let result = computed.compute(&model);

        assert_eq!(result, Value::String("John Doe".to_string()));
    }

    #[test]
    fn test_computed_field_collection() {
        let mut collection = ComputedFieldCollection::new();

        collection.add_fn("full_name", |model| {
            let first = get_string_field(model, "first_name").unwrap_or_default();
            let last = get_string_field(model, "last_name").unwrap_or_default();
            Value::String(format!("{} {}", first, last))
        });

        let model = create_person_model();
        let result = collection.apply(&model);

        let Value::Object(fields) = result else {
            unreachable!("Expected object");
        };
        let full_name = fields.iter().find(|(k, _)| k == "full_name");
        assert!(full_name.is_some());
        assert_eq!(full_name.unwrap().1, Value::String("John Doe".to_string()));
    }

    #[test]
    fn test_concat_computed() {
        let computed = ConcatComputed::new(
            "full_name",
            vec!["first_name".to_string(), "last_name".to_string()],
            " ",
        );

        let model = create_person_model();
        let result = computed.compute(&model);

        assert_eq!(result, Value::String("John Doe".to_string()));
    }

    #[test]
    fn test_compute_values() {
        let mut collection = ComputedFieldCollection::new();

        collection.add_fn("full_name", |model| {
            let first = get_string_field(model, "first_name").unwrap_or_default();
            let last = get_string_field(model, "last_name").unwrap_or_default();
            Value::String(format!("{} {}", first, last))
        });

        collection.add_fn("is_adult", |model| {
            let age = get_int_field(model, "age").unwrap_or(0);
            Value::Bool(age >= 18)
        });

        let model = create_person_model();
        let computed = collection.compute_values(&model);

        assert_eq!(computed.len(), 2);
        assert_eq!(
            computed.get("full_name"),
            Some(&Value::String("John Doe".to_string()))
        );
        assert_eq!(computed.get("is_adult"), Some(&Value::Bool(true)));
    }

    #[test]
    fn test_get_helper_functions() {
        let model = create_person_model();

        assert_eq!(
            get_string_field(&model, "first_name"),
            Some("John".to_string())
        );
        assert_eq!(get_int_field(&model, "age"), Some(30));
        assert_eq!(get_float_field(&model, "age"), Some(30.0));
        assert_eq!(get_bool_field(&model, "age"), None);
    }

    #[test]
    fn test_computed_field_with_description() {
        let computed = FnComputedField::new("full_name", |_| Value::Null)
            .description("Full name of the person")
            .return_type("str");

        // Use trait methods via ComputedField trait
        assert_eq!(
            ComputedField::description(&computed),
            Some("Full name of the person")
        );
        assert_eq!(ComputedField::return_type(&computed), Some("str"));
    }

    #[test]
    fn test_empty_collection() {
        let collection = ComputedFieldCollection::new();
        assert!(collection.is_empty());
        assert_eq!(collection.len(), 0);

        let model = create_person_model();
        let result = collection.apply(&model);
        assert_eq!(result, model);
    }
}
