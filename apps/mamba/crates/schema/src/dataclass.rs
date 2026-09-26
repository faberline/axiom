//! Dataclass-style validation support
//!
//! Provides utilities for working with Python dataclass-like structures
//! and automatic field inference.

use crate::constraints::{FieldDescriptor, NumericConstraints, StringConstraints};
use crate::types::TypeDescriptor;
use std::collections::HashMap;

// ============================================================================
// Field Info
// ============================================================================

/// Information about a field in a dataclass-like structure
#[derive(Debug, Clone)]
pub struct FieldInfo {
    /// Field name
    pub name: String,
    /// Python type annotation string
    pub annotation: String,
    /// Whether the field has a default value
    pub has_default: bool,
    /// Default value (as string representation)
    pub default_repr: Option<String>,
    /// Whether this is a ClassVar (should be excluded)
    pub is_class_var: bool,
    /// Whether this field is init-only
    pub init_only: bool,
    /// Field metadata
    pub metadata: HashMap<String, String>,
}

impl FieldInfo {
    /// Create a new field info
    pub fn new(name: impl Into<String>, annotation: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            annotation: annotation.into(),
            has_default: false,
            default_repr: None,
            is_class_var: false,
            init_only: false,
            metadata: HashMap::new(),
        }
    }

    /// Set default value
    pub fn default_value(mut self, repr: impl Into<String>) -> Self {
        self.has_default = true;
        self.default_repr = Some(repr.into());
        self
    }

    /// Mark as class var
    pub fn class_var(mut self) -> Self {
        self.is_class_var = true;
        self
    }

    /// Mark as init-only
    pub fn init_only(mut self) -> Self {
        self.init_only = true;
        self
    }

    /// Add metadata
    pub fn metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

// ============================================================================
// Type Inference
// ============================================================================

/// Infer a TypeDescriptor from a Python type annotation string
pub fn infer_type_from_annotation(annotation: &str) -> TypeDescriptor {
    let annotation = annotation.trim();

    // Handle Optional[T] and Union[T, None]
    if let Some(inner) = strip_optional(annotation) {
        return TypeDescriptor::Optional(Box::new(infer_type_from_annotation(inner)));
    }

    // Handle List[T]
    if let Some(inner) = strip_prefix_suffix(annotation, "List[", "]")
        .or_else(|| strip_prefix_suffix(annotation, "list[", "]"))
    {
        return TypeDescriptor::List {
            items: Box::new(infer_type_from_annotation(inner)),
            constraints: Default::default(),
        };
    }

    // Handle Dict[K, V]
    if let Some(inner) = strip_prefix_suffix(annotation, "Dict[", "]")
        .or_else(|| strip_prefix_suffix(annotation, "dict[", "]"))
    {
        // For now, treat as object with any additional properties
        if let Some((_, value_type)) = inner.split_once(',') {
            return TypeDescriptor::Object {
                fields: vec![],
                additional: Some(Box::new(infer_type_from_annotation(value_type.trim()))),
            };
        }
    }

    // Handle basic types
    match annotation {
        "str" | "String" => TypeDescriptor::String(StringConstraints::default()),
        "int" | "Integer" => TypeDescriptor::Int64(NumericConstraints::default()),
        "float" | "Float" | "Double" => TypeDescriptor::Float64(NumericConstraints::default()),
        "bool" | "Boolean" => TypeDescriptor::Bool,
        "bytes" | "Bytes" => TypeDescriptor::Bytes,
        "None" | "NoneType" => TypeDescriptor::Optional(Box::new(TypeDescriptor::Any)),
        "Any" => TypeDescriptor::Any,

        // Special format types
        "EmailStr" => TypeDescriptor::Email,
        "HttpUrl" | "AnyUrl" => TypeDescriptor::Url,
        "UUID" | "uuid.UUID" => TypeDescriptor::Uuid,
        "datetime" | "datetime.datetime" => TypeDescriptor::DateTime,
        "date" | "datetime.date" => TypeDescriptor::Date,
        "time" | "datetime.time" => TypeDescriptor::Time,

        // Default to any for unknown types
        _ => TypeDescriptor::Any,
    }
}

fn strip_optional(s: &str) -> Option<&str> {
    strip_prefix_suffix(s, "Optional[", "]").or_else(|| {
        // Handle Union[X, None]
        if let Some(inner) = strip_prefix_suffix(s, "Union[", "]") {
            let parts: Vec<&str> = inner.split(',').map(|p| p.trim()).collect();
            if parts.len() == 2 && parts[1] == "None" {
                return Some(parts[0]);
            }
        }
        None
    })
}

fn strip_prefix_suffix<'a>(s: &'a str, prefix: &str, suffix: &str) -> Option<&'a str> {
    s.strip_prefix(prefix)?.strip_suffix(suffix)
}

// ============================================================================
// Dataclass Definition
// ============================================================================

/// A dataclass definition
#[derive(Debug, Clone)]
pub struct DataclassDefinition {
    /// Class name
    pub name: String,
    /// Fields
    pub fields: Vec<FieldInfo>,
    /// Whether to freeze (make immutable)
    pub frozen: bool,
    /// Whether to generate ordering methods
    pub order: bool,
    /// Whether to generate __eq__
    pub eq: bool,
    /// Whether to generate __hash__
    pub hash: bool,
    /// Whether to generate __init__
    pub init: bool,
    /// Whether to generate __repr__
    pub repr: bool,
}

impl DataclassDefinition {
    /// Create a new dataclass definition
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            fields: Vec::new(),
            frozen: false,
            order: false,
            eq: true,
            hash: false,
            init: true,
            repr: true,
        }
    }

    /// Add a field
    pub fn field(mut self, field: FieldInfo) -> Self {
        self.fields.push(field);
        self
    }

    /// Set frozen
    pub fn frozen(mut self, frozen: bool) -> Self {
        self.frozen = frozen;
        self
    }

    /// Set order
    pub fn order(mut self, order: bool) -> Self {
        self.order = order;
        self
    }

    /// Convert to TypeDescriptor
    pub fn to_type_descriptor(&self) -> TypeDescriptor {
        let fields: Vec<FieldDescriptor> = self
            .fields
            .iter()
            .filter(|f| !f.is_class_var)
            .map(|f| {
                let type_desc = infer_type_from_annotation(&f.annotation);
                let mut fd = FieldDescriptor::new(&f.name, type_desc);
                if f.has_default {
                    fd = fd.optional();
                }
                fd
            })
            .collect();

        TypeDescriptor::Object {
            fields,
            additional: None,
        }
    }

    /// Get required field names
    pub fn required_fields(&self) -> Vec<&str> {
        self.fields
            .iter()
            .filter(|f| !f.has_default && !f.is_class_var)
            .map(|f| f.name.as_str())
            .collect()
    }

    /// Get optional field names
    pub fn optional_fields(&self) -> Vec<&str> {
        self.fields
            .iter()
            .filter(|f| f.has_default && !f.is_class_var)
            .map(|f| f.name.as_str())
            .collect()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_infer_basic_types() {
        assert!(matches!(
            infer_type_from_annotation("str"),
            TypeDescriptor::String(_)
        ));
        assert!(matches!(
            infer_type_from_annotation("int"),
            TypeDescriptor::Int64(_)
        ));
        assert!(matches!(
            infer_type_from_annotation("float"),
            TypeDescriptor::Float64(_)
        ));
        assert!(matches!(
            infer_type_from_annotation("bool"),
            TypeDescriptor::Bool
        ));
    }

    #[test]
    fn test_infer_optional() {
        let desc = infer_type_from_annotation("Optional[str]");
        assert!(matches!(desc, TypeDescriptor::Optional(_)));

        let desc = infer_type_from_annotation("Union[int, None]");
        assert!(matches!(desc, TypeDescriptor::Optional(_)));
    }

    #[test]
    fn test_infer_list() {
        let desc = infer_type_from_annotation("List[str]");
        assert!(matches!(desc, TypeDescriptor::List { .. }));

        let desc = infer_type_from_annotation("list[int]");
        assert!(matches!(desc, TypeDescriptor::List { .. }));
    }

    #[test]
    fn test_infer_special_types() {
        assert!(matches!(
            infer_type_from_annotation("EmailStr"),
            TypeDescriptor::Email
        ));
        assert!(matches!(
            infer_type_from_annotation("HttpUrl"),
            TypeDescriptor::Url
        ));
        assert!(matches!(
            infer_type_from_annotation("UUID"),
            TypeDescriptor::Uuid
        ));
    }

    #[test]
    fn test_field_info() {
        let field = FieldInfo::new("name", "str")
            .default_value("\"John\"")
            .metadata("description", "User name");

        assert_eq!(field.name, "name");
        assert_eq!(field.annotation, "str");
        assert!(field.has_default);
        assert_eq!(field.default_repr, Some("\"John\"".to_string()));
        assert_eq!(
            field.metadata.get("description"),
            Some(&"User name".to_string())
        );
    }

    #[test]
    fn test_dataclass_definition() {
        let dc = DataclassDefinition::new("User")
            .field(FieldInfo::new("name", "str"))
            .field(FieldInfo::new("age", "int").default_value("0"))
            .frozen(true);

        assert_eq!(dc.name, "User");
        assert_eq!(dc.fields.len(), 2);
        assert!(dc.frozen);

        let required = dc.required_fields();
        assert_eq!(required, vec!["name"]);

        let optional = dc.optional_fields();
        assert_eq!(optional, vec!["age"]);
    }

    #[test]
    fn test_dataclass_to_type_descriptor() {
        let dc = DataclassDefinition::new("User")
            .field(FieldInfo::new("name", "str"))
            .field(FieldInfo::new("email", "EmailStr"));

        let desc = dc.to_type_descriptor();
        let TypeDescriptor::Object { fields, .. } = desc else {
            unreachable!("Expected Object type descriptor");
        };
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].name, "name");
        assert_eq!(fields[1].name, "email");
    }
}
