//! Core type system for validation
//!
//! This module defines the type descriptors and value types used for validation.

use crate::constraints::{FieldDescriptor, ListConstraints, NumericConstraints, StringConstraints};

// ============================================================================
// Value Enum - Runtime values to be validated
// ============================================================================

/// Runtime value that can be validated
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// Null value
    Null,
    /// Boolean value
    Bool(bool),
    /// Integer value (i64)
    Int(i64),
    /// Float value (f64)
    Float(f64),
    /// String value
    String(String),
    /// Binary data
    Bytes(Vec<u8>),
    /// List/Array of values
    List(Vec<Value>),
    /// Object/Dictionary (key-value pairs)
    Object(Vec<(String, Value)>),
}

impl Value {
    /// Get human-readable type name for error messages
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Bool(_) => "boolean",
            Self::Int(_) => "integer",
            Self::Float(_) => "float",
            Self::String(_) => "string",
            Self::Bytes(_) => "bytes",
            Self::List(_) => "array",
            Self::Object(_) => "object",
        }
    }

    /// Check if value is null
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }
}

// ============================================================================
// TypeDescriptor - Type definitions for validation
// ============================================================================

/// Type descriptor for validation
///
/// This enum represents all possible types that can be validated,
/// from primitives (string, int, bool) to complex types (objects, unions).
#[derive(Debug, Clone)]
pub enum TypeDescriptor {
    // ========================================================================
    // Primitive Types
    // ========================================================================
    /// String type with constraints (length, pattern, format)
    String(StringConstraints),

    /// Integer type with numeric constraints (i64)
    Int64(NumericConstraints<i64>),

    /// Float type with numeric constraints (f64)
    Float64(NumericConstraints<f64>),

    /// Boolean type
    Bool,

    /// Null type
    Null,

    /// Binary data type
    Bytes,

    // ========================================================================
    // Collection Types
    // ========================================================================
    /// List/Array type with item type and constraints
    List {
        /// Type of items in the list
        items: Box<TypeDescriptor>,
        /// List constraints (min/max items, uniqueness)
        constraints: ListConstraints,
    },

    /// Tuple type (fixed-length ordered collection with specific types)
    Tuple {
        /// Types of items in the tuple (order matters)
        items: Vec<TypeDescriptor>,
    },

    /// Set type (unique items only)
    Set {
        /// Type of items in the set
        items: Box<TypeDescriptor>,
    },

    /// Object/Dictionary type with named fields
    Object {
        /// Field descriptors (name, type, required, default)
        fields: Vec<FieldDescriptor>,
        /// Type for additional properties not in fields
        additional: Option<Box<TypeDescriptor>>,
    },

    // ========================================================================
    // Special Types
    // ========================================================================
    /// Optional type (nullable)
    Optional(Box<TypeDescriptor>),

    /// Union type (value can be one of multiple types)
    Union {
        /// Possible types for this union
        variants: Vec<TypeDescriptor>,
        /// Whether null is allowed
        nullable: bool,
    },

    /// Enum type (value must match one of the allowed values)
    Enum {
        /// Allowed values
        values: Vec<Value>,
    },

    /// Literal type (value must exactly match one of the literal values)
    Literal {
        /// Literal values
        values: Vec<Value>,
    },

    // ========================================================================
    // Format Types (automatic string validation)
    // ========================================================================
    /// Email format (validated string)
    Email,

    /// URL format (validated string)
    Url,

    /// UUID format (validated string)
    Uuid,

    /// ISO 8601 DateTime format (validated string)
    DateTime,

    /// Date format YYYY-MM-DD (validated string)
    Date,

    /// Time format HH:MM:SS (validated string)
    Time,

    /// Decimal type (high precision numeric)
    Decimal(NumericConstraints<f64>),

    // ========================================================================
    // New Format Types (Phase 1 Enhancement)
    // ========================================================================
    /// IPv4 address format (validated string)
    Ipv4,

    /// IPv6 address format (validated string)
    Ipv6,

    /// Hostname format (validated string)
    Hostname,

    /// Fully Qualified Domain Name (validated string)
    Fqdn,

    /// E.164 phone number format (validated string)
    Phone,

    /// Base64 encoded string (validated string)
    Base64,

    /// URL-friendly slug (validated string)
    Slug,

    /// Valid JSON string (validated string)
    JsonString,

    // ========================================================================
    // BSON-specific types (behind "bson" feature)
    // ========================================================================
    #[cfg(feature = "bson")]
    /// MongoDB ObjectId
    ObjectId,

    #[cfg(feature = "bson")]
    /// BSON DateTime (UTC timestamp)
    BsonDateTime,

    #[cfg(feature = "bson")]
    /// BSON Decimal128 (high precision decimal)
    BsonDecimal128,

    #[cfg(feature = "bson")]
    /// BSON Binary data with subtype
    BsonBinary,

    // ========================================================================
    // Generic Type Parameter
    // ========================================================================
    /// Type parameter placeholder for generic types (e.g., "T", "K", "V")
    ///
    /// This variant is used as a placeholder in generic type definitions
    /// and should be substituted before validation.
    TypeParam(String),

    // ========================================================================
    // Any Type
    // ========================================================================
    /// Any type (no validation)
    Any,
}

impl TypeDescriptor {
    /// Get human-readable type name for error messages
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::String(_) => "string",
            Self::Int64(_) => "integer",
            Self::Float64(_) => "float",
            Self::Bool => "boolean",
            Self::Null => "null",
            Self::Bytes => "bytes",
            Self::List { .. } => "array",
            Self::Tuple { .. } => "tuple",
            Self::Set { .. } => "set",
            Self::Object { .. } => "object",
            Self::Optional(_) => "optional",
            Self::Union { .. } => "union",
            Self::Enum { .. } => "enum",
            Self::Literal { .. } => "literal",
            Self::Email => "email",
            Self::Url => "url",
            Self::Uuid => "uuid",
            Self::DateTime => "datetime",
            Self::Date => "date",
            Self::Time => "time",
            Self::Decimal(_) => "decimal",
            Self::Ipv4 => "ipv4",
            Self::Ipv6 => "ipv6",
            Self::Hostname => "hostname",
            Self::Fqdn => "fqdn",
            Self::Phone => "phone",
            Self::Base64 => "base64",
            Self::Slug => "slug",
            Self::JsonString => "json",
            #[cfg(feature = "bson")]
            Self::ObjectId => "objectid",
            #[cfg(feature = "bson")]
            Self::BsonDateTime => "bson_datetime",
            #[cfg(feature = "bson")]
            Self::BsonDecimal128 => "bson_decimal128",
            #[cfg(feature = "bson")]
            Self::BsonBinary => "bson_binary",
            Self::TypeParam(_) => "type_param",
            Self::Any => "any",
        }
    }
}

// ============================================================================
// RootModel - Allow non-Object root types (like Pydantic's RootModel)
// ============================================================================

/// A root model that allows any type at the root level
///
/// Similar to Pydantic's `RootModel[T]`, this allows validation of
/// non-object types (lists, primitives, etc.) at the root level.
///
/// # Example
///
/// ```rust,ignore
/// use schema::{RootModel, TypeDescriptor, Value, validate};
///
/// // Define a list of strings as root type
/// let model = RootModel::new(TypeDescriptor::List {
///     items: Box::new(TypeDescriptor::String(Default::default())),
///     constraints: Default::default(),
/// });
///
/// // Validate a JSON array directly
/// let data = Value::List(vec![
///     Value::String("hello".to_string()),
///     Value::String("world".to_string()),
/// ]);
///
/// assert!(model.validate(&data).is_ok());
/// ```
#[derive(Debug, Clone)]
pub struct RootModel {
    /// The root type descriptor
    root: TypeDescriptor,
    /// Optional model name for error messages
    name: Option<String>,
}

impl RootModel {
    /// Create a new root model with the given type
    pub fn new(root: TypeDescriptor) -> Self {
        Self { root, name: None }
    }

    /// Create a root model for a list type
    pub fn list(item_type: TypeDescriptor) -> Self {
        Self::new(TypeDescriptor::List {
            items: Box::new(item_type),
            constraints: Default::default(),
        })
    }

    /// Set a name for the model
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Get the root type descriptor
    pub fn root_type(&self) -> &TypeDescriptor {
        &self.root
    }

    /// Get the model name
    pub fn model_name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Validate a value against this root model
    pub fn validate(&self, value: &crate::Value) -> crate::ValidationResult<()> {
        crate::validators::validate(value, &self.root)
    }
}

impl From<TypeDescriptor> for RootModel {
    fn from(root: TypeDescriptor) -> Self {
        Self::new(root)
    }
}

// ============================================================================
// Conversions
// ============================================================================

#[cfg(feature = "serde")]
impl From<Value> for serde_json::Value {
    fn from(value: Value) -> Self {
        match value {
            Value::Null => serde_json::Value::Null,
            Value::Bool(b) => serde_json::Value::Bool(b),
            Value::Int(i) => serde_json::Value::Number(i.into()),
            Value::Float(f) => serde_json::Number::from_f64(f)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null),
            Value::String(s) => serde_json::Value::String(s),
            Value::Bytes(b) => {
                // Encode bytes as base64 string
                serde_json::Value::String(base64_encode(&b))
            }
            Value::List(items) => {
                serde_json::Value::Array(items.into_iter().map(Into::into).collect())
            }
            Value::Object(fields) => {
                serde_json::Value::Object(fields.into_iter().map(|(k, v)| (k, v.into())).collect())
            }
        }
    }
}

#[cfg(feature = "serde")]
fn base64_encode(data: &[u8]) -> String {
    use std::fmt::Write;
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut result = String::new();
    let mut i = 0;
    while i < data.len() {
        let b1 = data[i];
        let b2 = if i + 1 < data.len() { data[i + 1] } else { 0 };
        let b3 = if i + 2 < data.len() { data[i + 2] } else { 0 };

        let _ = write!(
            &mut result,
            "{}{}{}{}",
            CHARSET[(b1 >> 2) as usize] as char,
            CHARSET[(((b1 & 0x03) << 4) | (b2 >> 4)) as usize] as char,
            if i + 1 < data.len() {
                CHARSET[(((b2 & 0x0F) << 2) | (b3 >> 6)) as usize] as char
            } else {
                '='
            },
            if i + 2 < data.len() {
                CHARSET[(b3 & 0x3F) as usize] as char
            } else {
                '='
            },
        );

        i += 3;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_type_name() {
        assert_eq!(Value::Null.type_name(), "null");
        assert_eq!(Value::Bool(true).type_name(), "boolean");
        assert_eq!(Value::Int(42).type_name(), "integer");
        assert_eq!(Value::Float(3.14).type_name(), "float");
        assert_eq!(Value::String("test".to_string()).type_name(), "string");
        assert_eq!(Value::Bytes(vec![1, 2, 3]).type_name(), "bytes");
        assert_eq!(Value::List(vec![]).type_name(), "array");
        assert_eq!(Value::Object(vec![]).type_name(), "object");
    }

    #[test]
    fn test_value_is_null() {
        assert!(Value::Null.is_null());
        assert!(!Value::Bool(false).is_null());
        assert!(!Value::Int(0).is_null());
    }

    #[test]
    fn test_type_descriptor_type_name() {
        assert_eq!(
            TypeDescriptor::String(Default::default()).type_name(),
            "string"
        );
        assert_eq!(
            TypeDescriptor::Int64(Default::default()).type_name(),
            "integer"
        );
        assert_eq!(TypeDescriptor::Bool.type_name(), "boolean");
        assert_eq!(TypeDescriptor::Email.type_name(), "email");
        assert_eq!(TypeDescriptor::Url.type_name(), "url");
        assert_eq!(TypeDescriptor::Uuid.type_name(), "uuid");
    }

    #[test]
    fn test_root_model_list() {
        let model = RootModel::list(TypeDescriptor::String(Default::default())).name("StringList");

        assert_eq!(model.model_name(), Some("StringList"));
        assert!(matches!(model.root_type(), TypeDescriptor::List { .. }));

        // Valid: list of strings
        let valid = Value::List(vec![
            Value::String("a".to_string()),
            Value::String("b".to_string()),
        ]);
        assert!(model.validate(&valid).is_ok());

        // Invalid: list with non-string
        let invalid = Value::List(vec![Value::String("a".to_string()), Value::Int(42)]);
        assert!(model.validate(&invalid).is_err());
    }

    #[test]
    fn test_root_model_primitive() {
        // RootModel can be a primitive type
        let model = RootModel::new(TypeDescriptor::Int64(Default::default()));

        assert!(model.validate(&Value::Int(42)).is_ok());
        assert!(model.validate(&Value::String("42".to_string())).is_err());
    }

    #[test]
    fn test_root_model_from() {
        let model: RootModel = TypeDescriptor::Bool.into();
        assert!(model.validate(&Value::Bool(true)).is_ok());
    }
}
