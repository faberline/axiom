//! JSON Schema export for validation types
//!
//! Generates JSON Schema 2020-12 compatible schemas from TypeDescriptors
//! for OpenAPI 3.1 compatibility.

use crate::constraints::StringFormat;
use crate::types::{TypeDescriptor, Value};
use std::collections::HashMap;

// ============================================================================
// JSON Schema Types
// ============================================================================

/// JSON Schema representation
///
/// Supports JSON Schema 2020-12 / OpenAPI 3.1 specification.
#[derive(Debug, Clone, Default)]
pub struct JsonSchema {
    /// Schema type
    pub schema_type: Option<String>,
    /// Schema format
    pub format: Option<String>,
    /// Title
    pub title: Option<String>,
    /// Description
    pub description: Option<String>,
    /// Default value (as JSON string)
    pub default: Option<String>,
    /// Examples array (JSON Schema 2020-12)
    pub examples: Vec<String>,
    /// Enum values
    pub enum_values: Option<Vec<String>>,

    // String constraints
    /// Minimum string length
    pub min_length: Option<usize>,
    /// Maximum string length
    pub max_length: Option<usize>,
    /// Regex pattern
    pub pattern: Option<String>,

    // Numeric constraints
    /// Minimum value
    pub minimum: Option<f64>,
    /// Maximum value
    pub maximum: Option<f64>,
    /// Exclusive minimum
    pub exclusive_minimum: Option<f64>,
    /// Exclusive maximum
    pub exclusive_maximum: Option<f64>,
    /// Multiple of
    pub multiple_of: Option<f64>,

    // Array constraints
    /// Array items schema (None = not specified, Some = schema)
    pub items: Option<Box<JsonSchema>>,
    /// Literal boolean for items (JSON Schema 2020-12: false = no additional items allowed)
    /// When true, this takes precedence over `items` and outputs `"items": false`
    pub items_false: bool,
    /// Tuple items schema (JSON Schema 2020-12 prefixItems)
    pub prefix_items: Option<Vec<JsonSchema>>,
    /// Minimum items
    pub min_items: Option<usize>,
    /// Maximum items
    pub max_items: Option<usize>,
    /// Unique items
    pub unique_items: Option<bool>,

    // Object constraints
    /// Object properties
    pub properties: Option<HashMap<String, JsonSchema>>,
    /// Required field names
    pub required: Option<Vec<String>>,
    /// Additional properties schema
    pub additional_properties: Option<Box<JsonSchema>>,

    // Composition
    /// Any of (union types)
    pub any_of: Option<Vec<JsonSchema>>,
    /// All of (intersection types)
    pub all_of: Option<Vec<JsonSchema>>,
    /// One of (exclusive union)
    pub one_of: Option<Vec<JsonSchema>>,
    /// Not
    pub not: Option<Box<JsonSchema>>,

    // References (Phase 3 Enhancement)
    /// $ref - Reference to another schema
    pub schema_ref: Option<String>,
    /// $defs - Local schema definitions
    pub defs: Option<HashMap<String, JsonSchema>>,
    /// Discriminator for tagged unions (OpenAPI 3.1)
    pub discriminator: Option<Discriminator>,

    // Metadata (Phase 3 Enhancement)
    /// Mark field as deprecated
    pub deprecated: bool,
    /// Mark field as read-only
    pub read_only: bool,
    /// Mark field as write-only
    pub write_only: bool,

    /// Nullable
    pub nullable: bool,
}

/// Discriminator for tagged unions (OpenAPI 3.1)
#[derive(Debug, Clone, Default)]
pub struct Discriminator {
    /// Property name that holds the discriminator value
    pub property_name: String,
    /// Mapping of discriminator values to schema references
    pub mapping: Option<HashMap<String, String>>,
}

impl JsonSchema {
    /// Create a new empty schema
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a schema for a specific type
    pub fn with_type(schema_type: impl Into<String>) -> Self {
        Self {
            schema_type: Some(schema_type.into()),
            ..Default::default()
        }
    }

    /// Add format
    pub fn format(mut self, format: impl Into<String>) -> Self {
        self.format = Some(format.into());
        self
    }

    /// Add title
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Add description
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Set nullable
    pub fn nullable(mut self, nullable: bool) -> Self {
        self.nullable = nullable;
        self
    }

    /// Set deprecated
    pub fn deprecated(mut self, deprecated: bool) -> Self {
        self.deprecated = deprecated;
        self
    }

    /// Set read-only
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    /// Set write-only
    pub fn write_only(mut self, write_only: bool) -> Self {
        self.write_only = write_only;
        self
    }

    /// Set schema reference
    pub fn schema_ref(mut self, ref_path: impl Into<String>) -> Self {
        self.schema_ref = Some(ref_path.into());
        self
    }

    /// Add a definition
    pub fn add_def(mut self, name: impl Into<String>, schema: JsonSchema) -> Self {
        self.defs
            .get_or_insert_with(HashMap::new)
            .insert(name.into(), schema);
        self
    }

    /// Set discriminator
    pub fn discriminator(mut self, property_name: impl Into<String>) -> Self {
        self.discriminator = Some(Discriminator {
            property_name: property_name.into(),
            mapping: None,
        });
        self
    }

    /// Set discriminator with mapping
    pub fn discriminator_with_mapping(
        mut self,
        property_name: impl Into<String>,
        mapping: HashMap<String, String>,
    ) -> Self {
        self.discriminator = Some(Discriminator {
            property_name: property_name.into(),
            mapping: Some(mapping),
        });
        self
    }

    /// Convert to JSON string
    pub fn to_json(&self) -> String {
        let mut parts = Vec::new();

        // $ref takes precedence
        if let Some(ref r) = self.schema_ref {
            parts.push(format!(r#""$ref": "{}""#, r));
            // When $ref is present, other properties are ignored in JSON Schema
            return format!("{{{}}}", parts.join(", "));
        }

        if let Some(ref t) = self.schema_type {
            if self.nullable {
                parts.push(format!(r#""type": ["{}", "null"]"#, t));
            } else {
                parts.push(format!(r#""type": "{}""#, t));
            }
        }

        if let Some(ref f) = self.format {
            parts.push(format!(r#""format": "{}""#, f));
        }

        if let Some(ref t) = self.title {
            parts.push(format!(r#""title": "{}""#, t));
        }

        if let Some(ref d) = self.description {
            parts.push(format!(r#""description": "{}""#, d));
        }
        if let Some(ref default) = self.default {
            parts.push(format!(r#""default": {}"#, default));
        }

        // Metadata fields (Phase 3)
        if self.deprecated {
            parts.push(r#""deprecated": true"#.to_string());
        }
        if self.read_only {
            parts.push(r#""readOnly": true"#.to_string());
        }
        if self.write_only {
            parts.push(r#""writeOnly": true"#.to_string());
        }

        // String constraints
        if let Some(v) = self.min_length {
            parts.push(format!(r#""minLength": {}"#, v));
        }
        if let Some(v) = self.max_length {
            parts.push(format!(r#""maxLength": {}"#, v));
        }
        if let Some(ref p) = self.pattern {
            parts.push(format!(r#""pattern": "{}""#, p));
        }

        // Numeric constraints
        if let Some(v) = self.minimum {
            parts.push(format!(r#""minimum": {}"#, v));
        }
        if let Some(v) = self.maximum {
            parts.push(format!(r#""maximum": {}"#, v));
        }
        if let Some(v) = self.exclusive_minimum {
            parts.push(format!(r#""exclusiveMinimum": {}"#, v));
        }
        if let Some(v) = self.exclusive_maximum {
            parts.push(format!(r#""exclusiveMaximum": {}"#, v));
        }
        if let Some(v) = self.multiple_of {
            parts.push(format!(r#""multipleOf": {}"#, v));
        }

        // Array constraints
        if let Some(v) = self.min_items {
            parts.push(format!(r#""minItems": {}"#, v));
        }
        if let Some(v) = self.max_items {
            parts.push(format!(r#""maxItems": {}"#, v));
        }
        if let Some(v) = self.unique_items {
            parts.push(format!(r#""uniqueItems": {}"#, v));
        }

        // prefixItems for tuples (JSON Schema 2020-12)
        if let Some(ref prefix) = self.prefix_items {
            let items_json: Vec<_> = prefix.iter().map(|s| s.to_json()).collect();
            parts.push(format!(r#""prefixItems": [{}]"#, items_json.join(", ")));
        }

        // items: false takes precedence (for tuples with no additional items)
        if self.items_false {
            parts.push(r#""items": false"#.to_string());
        } else if let Some(ref items) = self.items {
            parts.push(format!(r#""items": {}"#, items.to_json()));
        }

        // Object properties
        if let Some(ref props) = self.properties {
            let props_json: Vec<_> = props
                .iter()
                .map(|(k, v)| format!(r#""{}": {}"#, k, v.to_json()))
                .collect();
            parts.push(format!(r#""properties": {{{}}}"#, props_json.join(", ")));
        }

        // Required fields
        if let Some(ref req) = self.required {
            let req_str: Vec<_> = req.iter().map(|s| format!(r#""{}""#, s)).collect();
            parts.push(format!(r#""required": [{}]"#, req_str.join(", ")));
        }

        // Additional properties
        if let Some(ref add) = self.additional_properties {
            parts.push(format!(r#""additionalProperties": {}"#, add.to_json()));
        }

        // Composition
        if let Some(ref any) = self.any_of {
            let schemas: Vec<_> = any.iter().map(|s| s.to_json()).collect();
            parts.push(format!(r#""anyOf": [{}]"#, schemas.join(", ")));
        }
        if let Some(ref all) = self.all_of {
            let schemas: Vec<_> = all.iter().map(|s| s.to_json()).collect();
            parts.push(format!(r#""allOf": [{}]"#, schemas.join(", ")));
        }
        if let Some(ref one) = self.one_of {
            let schemas: Vec<_> = one.iter().map(|s| s.to_json()).collect();
            parts.push(format!(r#""oneOf": [{}]"#, schemas.join(", ")));
        }

        // Discriminator (OpenAPI 3.1)
        if let Some(ref disc) = self.discriminator {
            let mut disc_parts = vec![format!(r#""propertyName": "{}""#, disc.property_name)];
            if let Some(ref mapping) = disc.mapping {
                let map_json: Vec<_> = mapping
                    .iter()
                    .map(|(k, v)| format!(r#""{}": "{}""#, k, v))
                    .collect();
                disc_parts.push(format!(r#""mapping": {{{}}}"#, map_json.join(", ")));
            }
            parts.push(format!(r#""discriminator": {{{}}}"#, disc_parts.join(", ")));
        }

        // $defs (local definitions)
        if let Some(ref defs) = self.defs {
            let defs_json: Vec<_> = defs
                .iter()
                .map(|(k, v)| format!(r#""{}": {}"#, k, v.to_json()))
                .collect();
            parts.push(format!(r#""$defs": {{{}}}"#, defs_json.join(", ")));
        }

        // Examples
        if !self.examples.is_empty() {
            parts.push(format!(r#""examples": [{}]"#, self.examples.join(", ")));
        }

        // Enum values
        if let Some(ref vals) = self.enum_values {
            parts.push(format!(r#""enum": [{}]"#, vals.join(", ")));
        }

        format!("{{{}}}", parts.join(", "))
    }
}

// ============================================================================
// Conversion from TypeDescriptor
// ============================================================================

/// Convert a TypeDescriptor to JSON Schema
pub fn type_descriptor_to_json_schema(desc: &TypeDescriptor) -> JsonSchema {
    match desc {
        TypeDescriptor::String(constraints) => {
            let mut schema = JsonSchema::with_type("string");
            if let Some(min) = constraints.min_length {
                schema.min_length = Some(min);
            }
            if let Some(max) = constraints.max_length {
                schema.max_length = Some(max);
            }
            if let Some(ref pattern) = constraints.pattern {
                schema.pattern = Some(pattern.clone());
            }
            if let Some(format) = &constraints.format {
                schema.format = Some(string_format_to_schema(format));
            }
            schema
        }

        TypeDescriptor::Int64(constraints) => {
            let mut schema = JsonSchema::with_type("integer");
            if let Some(min) = constraints.minimum {
                schema.minimum = Some(min as f64);
            }
            if let Some(max) = constraints.maximum {
                schema.maximum = Some(max as f64);
            }
            if let Some(min) = constraints.exclusive_minimum {
                schema.exclusive_minimum = Some(min as f64);
            }
            if let Some(max) = constraints.exclusive_maximum {
                schema.exclusive_maximum = Some(max as f64);
            }
            if let Some(multiple) = constraints.multiple_of {
                schema.multiple_of = Some(multiple as f64);
            }
            schema
        }

        TypeDescriptor::Float64(constraints) => {
            let mut schema = JsonSchema::with_type("number");
            if let Some(min) = constraints.minimum {
                schema.minimum = Some(min);
            }
            if let Some(max) = constraints.maximum {
                schema.maximum = Some(max);
            }
            if let Some(min) = constraints.exclusive_minimum {
                schema.exclusive_minimum = Some(min);
            }
            if let Some(max) = constraints.exclusive_maximum {
                schema.exclusive_maximum = Some(max);
            }
            if let Some(multiple) = constraints.multiple_of {
                schema.multiple_of = Some(multiple);
            }
            schema
        }

        TypeDescriptor::Bool => JsonSchema::with_type("boolean"),

        TypeDescriptor::List { items, constraints } => {
            let mut schema = JsonSchema::with_type("array");
            schema.items = Some(Box::new(type_descriptor_to_json_schema(items)));
            if let Some(min) = constraints.min_items {
                schema.min_items = Some(min);
            }
            if let Some(max) = constraints.max_items {
                schema.max_items = Some(max);
            }
            if constraints.unique_items {
                schema.unique_items = Some(true);
            }
            schema
        }

        TypeDescriptor::Object { fields, additional } => {
            let mut schema = JsonSchema::with_type("object");
            let mut properties = HashMap::new();
            let mut required = Vec::new();

            for field in fields {
                let mut property_schema = type_descriptor_to_json_schema(&field.type_desc);
                property_schema.title = field.title.clone();
                property_schema.description = field.description.clone();
                property_schema.examples = field.examples.iter().map(value_to_string).collect();
                property_schema.deprecated = field.deprecated;
                property_schema.read_only = field.read_only;
                property_schema.write_only = field.write_only;
                if let Some(default) = &field.default {
                    property_schema.default = Some(value_to_string(default));
                }
                properties.insert(field.serialization_name().to_string(), property_schema);
                if field.required {
                    required.push(field.serialization_name().to_string());
                }
            }

            schema.properties = Some(properties);
            if !required.is_empty() {
                schema.required = Some(required);
            }

            if let Some(ref add) = additional {
                schema.additional_properties = Some(Box::new(type_descriptor_to_json_schema(add)));
            }

            schema
        }

        TypeDescriptor::Optional(inner) => {
            let mut schema = type_descriptor_to_json_schema(inner);
            schema.nullable = true;
            schema
        }

        TypeDescriptor::Union { variants, nullable } => {
            let mut schema = JsonSchema::new();
            schema.any_of = Some(
                variants
                    .iter()
                    .map(type_descriptor_to_json_schema)
                    .collect(),
            );
            schema.nullable = *nullable;
            schema
        }

        TypeDescriptor::Literal { values } => {
            let mut schema = JsonSchema::new();
            schema.enum_values = Some(values.iter().map(value_to_string).collect());
            schema
        }

        TypeDescriptor::Enum { values } => {
            let mut schema = JsonSchema::new();
            schema.enum_values = Some(values.iter().map(value_to_string).collect());
            schema
        }

        TypeDescriptor::Tuple { items } => {
            let mut schema = JsonSchema::with_type("array");
            // JSON Schema 2020-12: Use prefixItems for tuples
            schema.prefix_items = Some(items.iter().map(type_descriptor_to_json_schema).collect());
            // Set items: false to prevent additional items beyond prefixItems
            schema.items_false = true;
            // Set exact length
            schema.min_items = Some(items.len());
            schema.max_items = Some(items.len());
            schema
        }

        TypeDescriptor::Set { items } => {
            let mut schema = JsonSchema::with_type("array");
            schema.items = Some(Box::new(type_descriptor_to_json_schema(items)));
            schema.unique_items = Some(true);
            schema
        }

        TypeDescriptor::Null => JsonSchema::with_type("null"),

        TypeDescriptor::Email => JsonSchema::with_type("string").format("email"),
        TypeDescriptor::Url => JsonSchema::with_type("string").format("uri"),
        TypeDescriptor::Uuid => JsonSchema::with_type("string").format("uuid"),
        TypeDescriptor::DateTime => JsonSchema::with_type("string").format("date-time"),
        TypeDescriptor::Date => JsonSchema::with_type("string").format("date"),
        TypeDescriptor::Time => JsonSchema::with_type("string").format("time"),
        TypeDescriptor::Bytes => JsonSchema::with_type("string").format("byte"),
        TypeDescriptor::Decimal(_) => JsonSchema::with_type("number"),
        TypeDescriptor::Any => JsonSchema::new(),

        // New format types (Phase 1)
        TypeDescriptor::Ipv4 => JsonSchema::with_type("string").format("ipv4"),
        TypeDescriptor::Ipv6 => JsonSchema::with_type("string").format("ipv6"),
        TypeDescriptor::Hostname => JsonSchema::with_type("string").format("hostname"),
        TypeDescriptor::Fqdn => JsonSchema::with_type("string").format("hostname"),
        TypeDescriptor::Phone => JsonSchema::with_type("string").format("phone"),
        TypeDescriptor::Base64 => JsonSchema::with_type("string").format("byte"),
        TypeDescriptor::Slug => JsonSchema::with_type("string").format("slug"),
        TypeDescriptor::JsonString => JsonSchema::with_type("string").format("json"),

        // BSON types (feature-gated)
        #[cfg(feature = "bson")]
        TypeDescriptor::ObjectId => JsonSchema::with_type("string").format("objectid"),
        #[cfg(feature = "bson")]
        TypeDescriptor::BsonDateTime => JsonSchema::with_type("string").format("date-time"),
        #[cfg(feature = "bson")]
        TypeDescriptor::BsonDecimal128 => JsonSchema::with_type("string"),
        #[cfg(feature = "bson")]
        TypeDescriptor::BsonBinary => JsonSchema::with_type("string").format("byte"),

        // Generic type parameter - generate $ref placeholder
        TypeDescriptor::TypeParam(name) => {
            JsonSchema::new().schema_ref(format!("#/$defs/{}", name))
        }
    }
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Int(i) => i.to_string(),
        Value::Float(f) => f.to_string(),
        Value::String(s) => quote_json_string(s),
        Value::Bytes(_) => "\"<bytes>\"".to_string(),
        Value::List(items) => {
            let values: Vec<String> = items.iter().map(value_to_string).collect();
            format!("[{}]", values.join(", "))
        }
        Value::Object(items) => {
            let values: Vec<String> = items
                .iter()
                .map(|(k, v)| format!("{}: {}", quote_json_string(k), value_to_string(v)))
                .collect();
            format!("{{{}}}", values.join(", "))
        }
    }
}

fn quote_json_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            ch if ch.is_control() => out.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}

fn string_format_to_schema(format: &StringFormat) -> String {
    match format {
        StringFormat::Email => "email".to_string(),
        StringFormat::Url => "uri".to_string(),
        StringFormat::Uuid => "uuid".to_string(),
        StringFormat::DateTime => "date-time".to_string(),
        StringFormat::Date => "date".to_string(),
        StringFormat::Time => "time".to_string(),
        // New formats (Phase 1)
        StringFormat::Ipv4 => "ipv4".to_string(),
        StringFormat::Ipv6 => "ipv6".to_string(),
        StringFormat::Hostname => "hostname".to_string(),
        StringFormat::Fqdn => "hostname".to_string(),
        StringFormat::Phone => "phone".to_string(),
        StringFormat::Base64 => "byte".to_string(),
        StringFormat::Slug => "slug".to_string(),
        StringFormat::Json => "json".to_string(),
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constraints::{FieldDescriptor, ListConstraints, StringConstraints};

    #[test]
    fn test_string_schema() {
        let schema = type_descriptor_to_json_schema(&TypeDescriptor::String(Default::default()));
        assert_eq!(schema.schema_type, Some("string".to_string()));
    }

    #[test]
    fn test_string_with_constraints() {
        let desc = TypeDescriptor::String(StringConstraints {
            min_length: Some(1),
            max_length: Some(100),
            pattern: Some("^[a-z]+$".to_string()),
            format: None,
        });
        let schema = type_descriptor_to_json_schema(&desc);

        assert_eq!(schema.min_length, Some(1));
        assert_eq!(schema.max_length, Some(100));
        assert_eq!(schema.pattern, Some("^[a-z]+$".to_string()));
    }

    #[test]
    fn test_integer_schema() {
        let schema = type_descriptor_to_json_schema(&TypeDescriptor::Int64(Default::default()));
        assert_eq!(schema.schema_type, Some("integer".to_string()));
    }

    #[test]
    fn test_array_schema() {
        let desc = TypeDescriptor::List {
            items: Box::new(TypeDescriptor::String(Default::default())),
            constraints: ListConstraints {
                min_items: Some(1),
                max_items: Some(10),
                unique_items: true,
            },
        };
        let schema = type_descriptor_to_json_schema(&desc);

        assert_eq!(schema.schema_type, Some("array".to_string()));
        assert_eq!(schema.min_items, Some(1));
        assert_eq!(schema.max_items, Some(10));
        assert_eq!(schema.unique_items, Some(true));
        assert!(schema.items.is_some());
    }

    #[test]
    fn test_object_schema() {
        let desc = TypeDescriptor::Object {
            fields: vec![
                FieldDescriptor::new("name", TypeDescriptor::String(Default::default())),
                FieldDescriptor::new("age", TypeDescriptor::Int64(Default::default())).optional(),
            ],
            additional: None,
        };
        let schema = type_descriptor_to_json_schema(&desc);

        assert_eq!(schema.schema_type, Some("object".to_string()));
        let props = schema.properties.unwrap();
        assert!(props.contains_key("name"));
        assert!(props.contains_key("age"));
        assert_eq!(schema.required, Some(vec!["name".to_string()]));
    }

    #[test]
    fn test_object_schema_preserves_field_default_and_description() {
        let desc = TypeDescriptor::Object {
            fields: vec![FieldDescriptor::new("active", TypeDescriptor::Bool)
                .default_value(Value::Bool(true))
                .description("Whether the item is enabled")],
            additional: None,
        };
        let schema = type_descriptor_to_json_schema(&desc);
        let props = schema.properties.unwrap();
        let active = props.get("active").unwrap();
        assert_eq!(
            active.description.as_deref(),
            Some("Whether the item is enabled")
        );
        assert_eq!(active.default.as_deref(), Some("true"));
        assert!(schema.required.is_none());
        assert!(active.to_json().contains(r#""default": true"#));
    }

    #[test]
    fn test_optional_schema() {
        let desc = TypeDescriptor::Optional(Box::new(TypeDescriptor::String(Default::default())));
        let schema = type_descriptor_to_json_schema(&desc);

        assert!(schema.nullable);
        assert_eq!(schema.schema_type, Some("string".to_string()));
    }

    #[test]
    fn test_email_format() {
        let schema = type_descriptor_to_json_schema(&TypeDescriptor::Email);
        assert_eq!(schema.format, Some("email".to_string()));
    }

    #[test]
    fn test_to_json() {
        let schema = JsonSchema::with_type("string")
            .format("email")
            .description("User email");

        let json = schema.to_json();
        assert!(json.contains(r#""type": "string""#));
        assert!(json.contains(r#""format": "email""#));
        assert!(json.contains(r#""description": "User email""#));
    }

    // ========================================================================
    // Phase 3: New JSON Schema Features Tests
    // ========================================================================

    #[test]
    fn test_tuple_prefix_items() {
        let desc = TypeDescriptor::Tuple {
            items: vec![
                TypeDescriptor::String(Default::default()),
                TypeDescriptor::Int64(Default::default()),
                TypeDescriptor::Bool,
            ],
        };
        let schema = type_descriptor_to_json_schema(&desc);

        assert_eq!(schema.schema_type, Some("array".to_string()));
        assert!(schema.prefix_items.is_some());
        // Verify prefix_items content
        assert!(schema.prefix_items.is_some());
        let prefix = schema.prefix_items.as_ref().unwrap();
        assert_eq!(prefix.len(), 3);
        assert_eq!(prefix[0].schema_type, Some("string".to_string()));
        assert_eq!(prefix[1].schema_type, Some("integer".to_string()));
        assert_eq!(prefix[2].schema_type, Some("boolean".to_string()));
        assert_eq!(schema.min_items, Some(3));
        assert_eq!(schema.max_items, Some(3));
        // Verify items: false is set (no additional items allowed)
        assert!(schema.items_false);

        // Test JSON output
        let json = schema.to_json();
        assert!(json.contains("\"prefixItems\":"));
        assert!(json.contains("\"items\": false"));
    }

    #[test]
    fn test_schema_ref() {
        let schema = JsonSchema::new().schema_ref("#/$defs/Address");
        let json = schema.to_json();

        assert!(json.contains("\"$ref\":"));
        assert!(json.contains("#/$defs/Address"));
        // $ref should be the only property
        assert!(!json.contains("\"type\":"));
    }

    #[test]
    fn test_schema_defs() {
        let address_schema = JsonSchema::with_type("object");
        let schema = JsonSchema::with_type("object").add_def("Address", address_schema);

        let json = schema.to_json();
        assert!(json.contains("\"$defs\":"));
        assert!(json.contains("\"Address\":"));
    }

    #[test]
    fn test_discriminator() {
        let schema = JsonSchema::new().discriminator("type");

        let json = schema.to_json();
        assert!(json.contains("\"discriminator\":"));
        assert!(json.contains("\"propertyName\": \"type\""));
    }

    #[test]
    fn test_discriminator_with_mapping() {
        let mut mapping = HashMap::new();
        mapping.insert("dog".to_string(), "#/$defs/Dog".to_string());
        mapping.insert("cat".to_string(), "#/$defs/Cat".to_string());

        let schema = JsonSchema::new().discriminator_with_mapping("pet_type", mapping);

        let json = schema.to_json();
        assert!(json.contains("\"discriminator\":"));
        assert!(json.contains("\"propertyName\": \"pet_type\""));
        assert!(json.contains("\"mapping\":"));
    }

    #[test]
    fn test_metadata_fields() {
        let schema = JsonSchema::with_type("string")
            .deprecated(true)
            .read_only(true)
            .write_only(false);

        let json = schema.to_json();
        assert!(json.contains("\"deprecated\": true"));
        assert!(json.contains("\"readOnly\": true"));
        assert!(!json.contains("\"writeOnly\":")); // false values not emitted
    }

    #[test]
    fn test_new_format_types_schema() {
        // Test IPv4
        let schema = type_descriptor_to_json_schema(&TypeDescriptor::Ipv4);
        assert_eq!(schema.format, Some("ipv4".to_string()));

        // Test IPv6
        let schema = type_descriptor_to_json_schema(&TypeDescriptor::Ipv6);
        assert_eq!(schema.format, Some("ipv6".to_string()));

        // Test Hostname
        let schema = type_descriptor_to_json_schema(&TypeDescriptor::Hostname);
        assert_eq!(schema.format, Some("hostname".to_string()));

        // Test Phone
        let schema = type_descriptor_to_json_schema(&TypeDescriptor::Phone);
        assert_eq!(schema.format, Some("phone".to_string()));

        // Test Slug
        let schema = type_descriptor_to_json_schema(&TypeDescriptor::Slug);
        assert_eq!(schema.format, Some("slug".to_string()));
    }
}
