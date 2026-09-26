//! Protobuf message generation from TypeDescriptor
//!
//! This module generates Protocol Buffers 3 message definitions from TypeDescriptor,
//! enabling a single source of truth for data models across REST and gRPC.
//!
//! # Example
//!
//! ```rust,ignore
//! use schema::protobuf::type_descriptor_to_proto;
//! use schema::{TypeDescriptor, FieldDescriptor};
//!
//! let user_type = TypeDescriptor::Object {
//!     fields: vec![
//!         FieldDescriptor::new("id", TypeDescriptor::Int64(Default::default())),
//!         FieldDescriptor::new("name", TypeDescriptor::String(Default::default())),
//!         FieldDescriptor::new("email", TypeDescriptor::Email).optional(),
//!     ],
//!     additional: None,
//! };
//!
//! let proto = type_descriptor_to_proto("User", &user_type);
//! // Output:
//! // message User {
//! //   int64 id = 1;
//! //   string name = 2;
//! //   optional string email = 3;
//! // }
//! ```

use crate::types::{TypeDescriptor, Value};
use std::collections::HashSet;

// ============================================================================
// Proto Generation Options
// ============================================================================

/// Options for protobuf generation
#[derive(Debug, Clone, Default)]
pub struct ProtoOptions {
    /// Package name for the proto file
    pub package: Option<String>,
    /// Syntax version (default: "proto3")
    pub syntax: String,
    /// Add field comments from descriptions
    pub include_comments: bool,
    /// Custom options to add
    pub options: Vec<(String, String)>,
}

impl ProtoOptions {
    /// Create new options with proto3 syntax
    pub fn new() -> Self {
        Self {
            syntax: "proto3".to_string(),
            ..Default::default()
        }
    }

    /// Set package name
    pub fn package(mut self, pkg: impl Into<String>) -> Self {
        self.package = Some(pkg.into());
        self
    }

    /// Include field comments
    pub fn with_comments(mut self) -> Self {
        self.include_comments = true;
        self
    }

    /// Add a custom option
    pub fn option(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.options.push((name.into(), value.into()));
        self
    }
}

// ============================================================================
// Proto File Generation
// ============================================================================

/// Generate a complete .proto file with multiple messages
pub fn generate_proto_file(messages: &[(&str, &TypeDescriptor)], options: &ProtoOptions) -> String {
    let mut output = String::new();

    // Syntax
    output.push_str(&format!("syntax = \"{}\";\n\n", options.syntax));

    // Package
    if let Some(ref pkg) = options.package {
        output.push_str(&format!("package {};\n\n", pkg));
    }

    // Options
    for (name, value) in &options.options {
        output.push_str(&format!("option {} = \"{}\";\n", name, value));
    }
    if !options.options.is_empty() {
        output.push('\n');
    }

    // Collect nested messages
    let mut generated: HashSet<String> = HashSet::new();

    // Generate each message
    for (name, desc) in messages {
        if !generated.contains(*name) {
            let (msg, nested) = generate_message_with_nested(name, desc, options, &mut generated);
            output.push_str(&nested);
            output.push_str(&msg);
            output.push('\n');
            generated.insert(name.to_string());
        }
    }

    output.trim_end().to_string()
}

/// Generate a single protobuf message definition
pub fn type_descriptor_to_proto(name: &str, desc: &TypeDescriptor) -> String {
    let options = ProtoOptions::new();
    let mut generated = HashSet::new();
    let (msg, nested) = generate_message_with_nested(name, desc, &options, &mut generated);
    format!("{}{}", nested, msg).trim().to_string()
}

/// Generate message with any nested message definitions
fn generate_message_with_nested(
    name: &str,
    desc: &TypeDescriptor,
    options: &ProtoOptions,
    generated: &mut HashSet<String>,
) -> (String, String) {
    let mut nested_output = String::new();
    let mut output = String::new();

    match desc {
        TypeDescriptor::Object { fields, .. } => {
            output.push_str(&format!("message {} {{\n", name));

            for (idx, field) in fields.iter().enumerate() {
                let field_num = idx + 1;

                // Comment
                if options.include_comments {
                    if let Some(ref desc) = field.description {
                        output.push_str(&format!("  // {}\n", desc));
                    }
                }

                // Check for nested objects that need separate message definitions
                let (proto_type, nested) =
                    type_to_proto_with_nested(&field.type_desc, &field.name, generated);

                if !nested.is_empty() {
                    nested_output.push_str(&nested);
                    nested_output.push('\n');
                }

                // Optional prefix for proto3
                let optional = if !field.required { "optional " } else { "" };

                output.push_str(&format!(
                    "  {}{} {} = {};\n",
                    optional, proto_type, field.name, field_num
                ));
            }

            output.push_str("}\n");
        }
        _ => {
            // For non-object types, wrap in a message with a single 'value' field
            output.push_str(&format!("message {} {{\n", name));
            let (proto_type, _) = type_to_proto_with_nested(desc, "value", generated);
            output.push_str(&format!("  {} value = 1;\n", proto_type));
            output.push_str("}\n");
        }
    }

    (output, nested_output)
}

/// Convert TypeDescriptor to proto type string, returning nested definitions if needed
fn type_to_proto_with_nested(
    desc: &TypeDescriptor,
    field_name: &str,
    generated: &mut HashSet<String>,
) -> (String, String) {
    match desc {
        // Primitive types
        TypeDescriptor::String(_) => ("string".to_string(), String::new()),
        TypeDescriptor::Int64(_) => ("int64".to_string(), String::new()),
        TypeDescriptor::Float64(_) => ("double".to_string(), String::new()),
        TypeDescriptor::Bool => ("bool".to_string(), String::new()),
        TypeDescriptor::Bytes => ("bytes".to_string(), String::new()),
        TypeDescriptor::Null => ("google.protobuf.NullValue".to_string(), String::new()),

        // Format types (all map to string in proto)
        TypeDescriptor::Email
        | TypeDescriptor::Url
        | TypeDescriptor::Uuid
        | TypeDescriptor::DateTime
        | TypeDescriptor::Date
        | TypeDescriptor::Time
        | TypeDescriptor::Ipv4
        | TypeDescriptor::Ipv6
        | TypeDescriptor::Hostname
        | TypeDescriptor::Fqdn
        | TypeDescriptor::Phone
        | TypeDescriptor::Base64
        | TypeDescriptor::Slug
        | TypeDescriptor::JsonString => ("string".to_string(), String::new()),

        TypeDescriptor::Decimal(_) => ("string".to_string(), String::new()), // Decimal as string for precision

        // Collection types
        TypeDescriptor::List { items, .. } => {
            let (item_type, nested) = type_to_proto_with_nested(items, field_name, generated);
            (format!("repeated {}", item_type), nested)
        }

        TypeDescriptor::Set { items } => {
            let (item_type, nested) = type_to_proto_with_nested(items, field_name, generated);
            (format!("repeated {}", item_type), nested)
        }

        TypeDescriptor::Tuple { items } => {
            // Tuples become a nested message with indexed fields
            let msg_name = to_pascal_case(&format!("{}_tuple", field_name));
            if generated.contains(&msg_name) {
                return (msg_name, String::new());
            }
            generated.insert(msg_name.clone());

            let mut nested = format!("message {} {{\n", msg_name);
            for (idx, item) in items.iter().enumerate() {
                let (item_type, item_nested) =
                    type_to_proto_with_nested(item, &format!("item{}", idx), generated);
                if !item_nested.is_empty() {
                    nested = format!("{}\n{}", item_nested, nested);
                }
                nested.push_str(&format!("  {} item{} = {};\n", item_type, idx, idx + 1));
            }
            nested.push_str("}\n");
            (msg_name, nested)
        }

        // Object becomes a nested message
        TypeDescriptor::Object { fields, .. } => {
            let msg_name = to_pascal_case(field_name);
            if generated.contains(&msg_name) {
                return (msg_name, String::new());
            }
            generated.insert(msg_name.clone());

            let mut nested = format!("message {} {{\n", msg_name);
            let mut sub_nested = String::new();

            for (idx, field) in fields.iter().enumerate() {
                let (field_type, field_nested) =
                    type_to_proto_with_nested(&field.type_desc, &field.name, generated);

                if !field_nested.is_empty() {
                    sub_nested.push_str(&field_nested);
                    sub_nested.push('\n');
                }

                let optional = if !field.required { "optional " } else { "" };
                nested.push_str(&format!(
                    "  {}{} {} = {};\n",
                    optional,
                    field_type,
                    field.name,
                    idx + 1
                ));
            }
            nested.push_str("}\n");

            (msg_name, format!("{}{}", sub_nested, nested))
        }

        // Optional wraps inner type
        TypeDescriptor::Optional(inner) => {
            let (inner_type, nested) = type_to_proto_with_nested(inner, field_name, generated);
            // In proto3, use 'optional' keyword
            (inner_type, nested)
        }

        // Union becomes oneof
        TypeDescriptor::Union { variants, .. } => {
            let msg_name = to_pascal_case(&format!("{}_union", field_name));
            if generated.contains(&msg_name) {
                return (msg_name, String::new());
            }
            generated.insert(msg_name.clone());

            let mut nested = format!("message {} {{\n  oneof value {{\n", msg_name);
            let mut sub_nested = String::new();

            for (idx, variant) in variants.iter().enumerate() {
                let variant_name = format!("option{}", idx);
                let (variant_type, variant_nested) =
                    type_to_proto_with_nested(variant, &variant_name, generated);

                if !variant_nested.is_empty() {
                    sub_nested.push_str(&variant_nested);
                    sub_nested.push('\n');
                }

                nested.push_str(&format!(
                    "    {} {} = {};\n",
                    variant_type,
                    variant_name,
                    idx + 1
                ));
            }
            nested.push_str("  }\n}\n");

            (msg_name, format!("{}{}", sub_nested, nested))
        }

        // Enum
        TypeDescriptor::Enum { values } | TypeDescriptor::Literal { values } => {
            let enum_name = to_pascal_case(&format!("{}_enum", field_name));
            if generated.contains(&enum_name) {
                return (enum_name, String::new());
            }
            generated.insert(enum_name.clone());

            let mut nested = format!("enum {} {{\n", enum_name);
            nested.push_str(&format!(
                "  {}_UNSPECIFIED = 0;\n",
                to_screaming_snake_case(field_name)
            ));

            for (idx, value) in values.iter().enumerate() {
                let value_name = value_to_enum_name(value);
                nested.push_str(&format!("  {} = {};\n", value_name, idx + 1));
            }
            nested.push_str("}\n");

            (enum_name, nested)
        }

        // Any type
        TypeDescriptor::Any => ("google.protobuf.Any".to_string(), String::new()),

        // Type parameter (should be resolved before proto generation)
        TypeDescriptor::TypeParam(name) => (name.clone(), String::new()),

        // BSON types (feature-gated)
        #[cfg(feature = "bson")]
        TypeDescriptor::ObjectId => ("string".to_string(), String::new()),
        #[cfg(feature = "bson")]
        TypeDescriptor::BsonDateTime => ("google.protobuf.Timestamp".to_string(), String::new()),
        #[cfg(feature = "bson")]
        TypeDescriptor::BsonDecimal128 => ("string".to_string(), String::new()),
        #[cfg(feature = "bson")]
        TypeDescriptor::BsonBinary => ("bytes".to_string(), String::new()),
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Convert string to PascalCase
fn to_pascal_case(s: &str) -> String {
    s.split(|c: char| c == '_' || c == '-' || c == ' ')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect::<String>(),
                None => String::new(),
            }
        })
        .collect()
}

/// Convert string to SCREAMING_SNAKE_CASE
fn to_screaming_snake_case(s: &str) -> String {
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            result.push('_');
        }
        result.push(c.to_ascii_uppercase());
    }
    result
}

/// Convert a Value to an enum variant name
fn value_to_enum_name(value: &Value) -> String {
    match value {
        Value::String(s) => to_screaming_snake_case(s),
        Value::Int(i) => format!("VALUE_{}", i),
        Value::Bool(b) => {
            if *b {
                "TRUE".to_string()
            } else {
                "FALSE".to_string()
            }
        }
        _ => "UNKNOWN".to_string(),
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constraints::FieldDescriptor;

    #[test]
    fn test_simple_message() {
        let desc = TypeDescriptor::Object {
            fields: vec![
                FieldDescriptor::new("id", TypeDescriptor::Int64(Default::default())),
                FieldDescriptor::new("name", TypeDescriptor::String(Default::default())),
            ],
            additional: None,
        };

        let proto = type_descriptor_to_proto("User", &desc);

        assert!(proto.contains("message User {"));
        assert!(proto.contains("int64 id = 1;"));
        assert!(proto.contains("string name = 2;"));
    }

    #[test]
    fn test_optional_fields() {
        let desc = TypeDescriptor::Object {
            fields: vec![
                FieldDescriptor::new("id", TypeDescriptor::Int64(Default::default())),
                FieldDescriptor::new("email", TypeDescriptor::Email).optional(),
            ],
            additional: None,
        };

        let proto = type_descriptor_to_proto("User", &desc);

        assert!(proto.contains("int64 id = 1;"));
        assert!(proto.contains("optional string email = 2;"));
    }

    #[test]
    fn test_repeated_field() {
        let desc = TypeDescriptor::Object {
            fields: vec![FieldDescriptor::new(
                "tags",
                TypeDescriptor::List {
                    items: Box::new(TypeDescriptor::String(Default::default())),
                    constraints: Default::default(),
                },
            )],
            additional: None,
        };

        let proto = type_descriptor_to_proto("Post", &desc);

        assert!(proto.contains("repeated string tags = 1;"));
    }

    #[test]
    fn test_nested_message() {
        let address_type = TypeDescriptor::Object {
            fields: vec![
                FieldDescriptor::new("street", TypeDescriptor::String(Default::default())),
                FieldDescriptor::new("city", TypeDescriptor::String(Default::default())),
            ],
            additional: None,
        };

        let desc = TypeDescriptor::Object {
            fields: vec![
                FieldDescriptor::new("name", TypeDescriptor::String(Default::default())),
                FieldDescriptor::new("address", address_type),
            ],
            additional: None,
        };

        let proto = type_descriptor_to_proto("User", &desc);

        assert!(proto.contains("message Address {"));
        assert!(proto.contains("message User {"));
        assert!(proto.contains("Address address = 2;"));
    }

    #[test]
    fn test_enum_field() {
        let desc = TypeDescriptor::Object {
            fields: vec![FieldDescriptor::new(
                "status",
                TypeDescriptor::Enum {
                    values: vec![
                        Value::String("pending".to_string()),
                        Value::String("active".to_string()),
                        Value::String("inactive".to_string()),
                    ],
                },
            )],
            additional: None,
        };

        let proto = type_descriptor_to_proto("User", &desc);

        assert!(proto.contains("enum StatusEnum {"));
        assert!(proto.contains("STATUS_UNSPECIFIED = 0;"));
        assert!(proto.contains("PENDING = 1;"));
        assert!(proto.contains("ACTIVE = 2;"));
        assert!(proto.contains("INACTIVE = 3;"));
    }

    #[test]
    fn test_proto_file_generation() {
        let user = TypeDescriptor::Object {
            fields: vec![
                FieldDescriptor::new("id", TypeDescriptor::Int64(Default::default())),
                FieldDescriptor::new("name", TypeDescriptor::String(Default::default())),
            ],
            additional: None,
        };

        let options = ProtoOptions::new()
            .package("example.v1")
            .option("go_package", "example.com/api/v1");

        let proto = generate_proto_file(&[("User", &user)], &options);

        assert!(proto.contains("syntax = \"proto3\";"));
        assert!(proto.contains("package example.v1;"));
        assert!(proto.contains("option go_package = \"example.com/api/v1\";"));
        assert!(proto.contains("message User {"));
    }

    #[test]
    fn test_format_types_map_to_string() {
        let desc = TypeDescriptor::Object {
            fields: vec![
                FieldDescriptor::new("email", TypeDescriptor::Email),
                FieldDescriptor::new("url", TypeDescriptor::Url),
                FieldDescriptor::new("uuid", TypeDescriptor::Uuid),
                FieldDescriptor::new("ip", TypeDescriptor::Ipv4),
            ],
            additional: None,
        };

        let proto = type_descriptor_to_proto("Contact", &desc);

        // All format types should map to string
        assert!(proto.contains("string email = 1;"));
        assert!(proto.contains("string url = 2;"));
        assert!(proto.contains("string uuid = 3;"));
        assert!(proto.contains("string ip = 4;"));
    }

    #[test]
    fn test_union_to_oneof() {
        let desc = TypeDescriptor::Object {
            fields: vec![FieldDescriptor::new(
                "result",
                TypeDescriptor::Union {
                    variants: vec![
                        TypeDescriptor::String(Default::default()),
                        TypeDescriptor::Int64(Default::default()),
                    ],
                    nullable: false,
                },
            )],
            additional: None,
        };

        let proto = type_descriptor_to_proto("Response", &desc);

        assert!(proto.contains("message ResultUnion {"));
        assert!(proto.contains("oneof value {"));
        assert!(proto.contains("string option0 = 1;"));
        assert!(proto.contains("int64 option1 = 2;"));
    }
}
