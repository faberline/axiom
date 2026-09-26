//! Input validation for MongoDB operations
//!
//! This module provides security-focused validation for MongoDB inputs to prevent
//! NoSQL injection attacks and other security vulnerabilities.
//!
//! # Security Features
//! - Collection name validation (prevents system collection access)
//! - Field name validation (prevents operator injection)
//! - Context-aware ObjectId parsing (prevents auto-conversion attacks)
//! - Type validation for document schemas

use crate::Result;
use bson::Bson;
use mamba_core::DataBridgeError;
use std::collections::HashMap;

/// Maximum allowed length for collection names (MongoDB limit is 255, we're more conservative)
const MAX_COLLECTION_NAME_LENGTH: usize = 120;

/// Maximum allowed length for field names
const MAX_FIELD_NAME_LENGTH: usize = 1024;

/// Validated collection name that prevents injection attacks
///
/// # Security Guarantees
/// - Not empty
/// - Maximum 120 characters
/// - No null bytes
/// - No "system." prefix (system collections)
/// - No $ characters (special operators)
/// - Warns on suspicious patterns (.., //)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedCollectionName {
    name: String,
}

impl ValidatedCollectionName {
    /// Creates a new validated collection name
    ///
    /// # Errors
    /// Returns ValidationError if:
    /// - Name is empty
    /// - Name exceeds MAX_COLLECTION_NAME_LENGTH
    /// - Name contains null bytes
    /// - Name starts with "system."
    /// - Name contains $ characters
    pub fn new(name: &str) -> Result<Self> {
        // Check: Not empty
        if name.is_empty() {
            return Err(DataBridgeError::Validation(
                "Collection name cannot be empty".to_string(),
            ));
        }

        // Check: Length limit
        if name.len() > MAX_COLLECTION_NAME_LENGTH {
            return Err(DataBridgeError::Validation(format!(
                "Collection name exceeds maximum length of {} characters: '{}'",
                MAX_COLLECTION_NAME_LENGTH, name
            )));
        }

        // Check: No null bytes
        if name.contains('\0') {
            return Err(DataBridgeError::Validation(
                "Collection name cannot contain null bytes".to_string(),
            ));
        }

        // Check: No "system." prefix (reserved for system collections)
        if name.starts_with("system.") {
            return Err(DataBridgeError::Validation(format!(
                "Collection name cannot start with 'system.' (reserved): '{}'",
                name
            )));
        }

        // Check: No $ characters (special MongoDB operators)
        if name.contains('$') {
            return Err(DataBridgeError::Validation(format!(
                "Collection name cannot contain '$' character: '{}'",
                name
            )));
        }

        // Warn on suspicious patterns (but allow them)
        if name.contains("..") || name.contains("//") {
            eprintln!(
                "WARNING: Collection name contains suspicious pattern: '{}'",
                name
            );
        }

        Ok(ValidatedCollectionName {
            name: name.to_string(),
        })
    }

    /// Returns the validated collection name as a string slice
    pub fn as_str(&self) -> &str {
        &self.name
    }

    /// Consumes the ValidatedCollectionName and returns the inner String
    pub fn into_string(self) -> String {
        self.name
    }
}

impl AsRef<str> for ValidatedCollectionName {
    fn as_ref(&self) -> &str {
        &self.name
    }
}

impl std::fmt::Display for ValidatedCollectionName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

/// Validated field name that prevents operator injection
///
/// # Security Guarantees
/// - Not empty
/// - Maximum 1024 characters
/// - No null bytes
/// - No $ prefix (except in specific update operators)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedFieldName {
    name: String,
    allow_operators: bool,
}

impl ValidatedFieldName {
    /// Creates a new validated field name
    ///
    /// # Arguments
    /// * `name` - The field name to validate
    /// * `allow_operators` - If true, allows $ prefix for update operators like $set, $inc
    ///
    /// # Errors
    /// Returns ValidationError if:
    /// - Name is empty
    /// - Name exceeds MAX_FIELD_NAME_LENGTH
    /// - Name contains null bytes
    /// - Name starts with $ and allow_operators is false
    pub fn new(name: &str, allow_operators: bool) -> Result<Self> {
        // Check: Not empty
        if name.is_empty() {
            return Err(DataBridgeError::Validation(
                "Field name cannot be empty".to_string(),
            ));
        }

        // Check: Length limit
        if name.len() > MAX_FIELD_NAME_LENGTH {
            return Err(DataBridgeError::Validation(format!(
                "Field name exceeds maximum length of {} characters",
                MAX_FIELD_NAME_LENGTH
            )));
        }

        // Check: No null bytes
        if name.contains('\0') {
            return Err(DataBridgeError::Validation(
                "Field name cannot contain null bytes".to_string(),
            ));
        }

        // Check: $ prefix (only allowed for update operators)
        if name.starts_with('$') && !allow_operators {
            return Err(DataBridgeError::Validation(format!(
                "Field name cannot start with '$' (reserved for operators): '{}'",
                name
            )));
        }

        // If $ is allowed, verify it's a known operator
        if name.starts_with('$') && allow_operators && !Self::is_known_operator(name) {
            eprintln!("WARNING: Unknown MongoDB operator: '{}'", name);
        }

        Ok(ValidatedFieldName {
            name: name.to_string(),
            allow_operators,
        })
    }

    /// Checks if the name is a known MongoDB operator
    fn is_known_operator(name: &str) -> bool {
        // Common update operators
        const UPDATE_OPERATORS: &[&str] = &[
            "$set",
            "$unset",
            "$inc",
            "$mul",
            "$rename",
            "$setOnInsert",
            "$min",
            "$max",
            "$currentDate",
            "$addToSet",
            "$pop",
            "$pull",
            "$push",
            "$pullAll",
            "$each",
            "$slice",
            "$sort",
            "$position",
        ];

        // Query operators (should not appear in field names, but listed for completeness)
        const QUERY_OPERATORS: &[&str] = &[
            "$eq",
            "$ne",
            "$gt",
            "$gte",
            "$lt",
            "$lte",
            "$in",
            "$nin",
            "$and",
            "$or",
            "$not",
            "$nor",
            "$exists",
            "$type",
            "$mod",
            "$regex",
            "$text",
            "$where",
            "$expr",
            "$jsonSchema",
            "$all",
            "$elemMatch",
            "$size",
        ];

        // Aggregation operators
        const AGGREGATION_OPERATORS: &[&str] = &[
            "$match",
            "$group",
            "$project",
            "$sort",
            "$limit",
            "$skip",
            "$unwind",
            "$lookup",
            "$out",
            "$merge",
            "$addFields",
        ];

        UPDATE_OPERATORS.contains(&name)
            || QUERY_OPERATORS.contains(&name)
            || AGGREGATION_OPERATORS.contains(&name)
    }

    /// Returns the validated field name as a string slice
    pub fn as_str(&self) -> &str {
        &self.name
    }

    /// Consumes the ValidatedFieldName and returns the inner String
    pub fn into_string(self) -> String {
        self.name
    }
}

impl AsRef<str> for ValidatedFieldName {
    fn as_ref(&self) -> &str {
        &self.name
    }
}

impl std::fmt::Display for ValidatedFieldName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

/// Context-aware ObjectId parser that prevents auto-conversion attacks
///
/// # Security Model
/// Prevents NoSQL injection via automatic ObjectId conversion by requiring
/// explicit type hints or wrapper classes.
pub struct ObjectIdParser;

impl ObjectIdParser {
    /// Checks if a string should be converted to ObjectId based on type hint
    ///
    /// # Arguments
    /// * `value` - The string value to check
    /// * `type_name` - Optional type hint (e.g., "ObjectId", "PydanticObjectId")
    ///
    /// # Returns
    /// - `true` if the value should be converted to ObjectId
    /// - `false` if it should remain a string
    ///
    /// # Security
    /// This function prevents auto-conversion attacks by requiring explicit type hints.
    /// Without type hints, even valid ObjectId strings (24 hex chars) remain as strings.
    pub fn should_convert_to_objectid(value: &str, type_name: Option<&str>) -> bool {
        // First check: Is it a valid ObjectId format?
        if !Self::is_valid_objectid_format(value) {
            return false;
        }

        // Second check: Do we have explicit type hint?
        match type_name {
            Some(name) => Self::is_objectid_type(name),
            None => false, // No type hint = no auto-conversion (security!)
        }
    }

    /// Checks if a string has valid ObjectId format (24 hex characters)
    fn is_valid_objectid_format(value: &str) -> bool {
        value.len() == 24 && value.chars().all(|c| c.is_ascii_hexdigit())
    }

    /// Checks if a type name indicates an ObjectId type
    fn is_objectid_type(type_name: &str) -> bool {
        matches!(
            type_name,
            "ObjectId" | "PydanticObjectId" | "BsonObjectId" | "MongoObjectId"
        )
    }
}

/// Dangerous MongoDB operators that should be blocked
const DANGEROUS_OPERATORS: &[&str] = &[
    "$where",       // JavaScript execution
    "$function",    // JavaScript execution
    "$accumulator", // Custom JavaScript in aggregation
];

/// Validates a MongoDB query document for dangerous operators
///
/// # Arguments
/// * `query` - The query document as BSON
///
/// # Errors
/// Returns ValidationError if dangerous operators are detected
pub fn validate_query(query: &bson::Bson) -> Result<()> {
    use bson::Bson;

    match query {
        Bson::Document(doc) => {
            for (key, value) in doc.iter() {
                // Check for dangerous operators
                if DANGEROUS_OPERATORS.contains(&key.as_str()) {
                    return Err(DataBridgeError::Validation(format!(
                        "Dangerous operator '{}' is not allowed for security reasons",
                        key
                    )));
                }

                // Recursively validate nested documents
                if let Bson::Document(_) = value {
                    validate_query(value)?;
                }

                // Validate arrays
                if let Bson::Array(arr) = value {
                    for item in arr {
                        validate_query(item)?;
                    }
                }
            }
            Ok(())
        }
        Bson::Array(arr) => {
            for item in arr {
                validate_query(item)?;
            }
            Ok(())
        }
        _ => Ok(()), // Primitive types are safe
    }
}

// =====================
// BSON ↔ Value Conversion
// =====================

/// Convert BSON to schema::Value for common validation
///
/// This conversion enables using the unified validation engine from cclab-shield
/// while preserving MongoDB-specific security validations.
#[allow(dead_code)]
fn bson_to_validation_value(bson: &Bson) -> schema::Value {
    match bson {
        Bson::Double(f) => schema::Value::Float(*f),
        Bson::String(s) => schema::Value::String(s.clone()),
        Bson::Array(arr) => {
            schema::Value::List(arr.iter().map(bson_to_validation_value).collect())
        }
        Bson::Document(doc) => schema::Value::Object(
            doc.iter()
                .map(|(k, v)| (k.clone(), bson_to_validation_value(v)))
                .collect(),
        ),
        Bson::Boolean(b) => schema::Value::Bool(*b),
        Bson::Null => schema::Value::Null,
        Bson::Int32(i) => schema::Value::Int(*i as i64),
        Bson::Int64(i) => schema::Value::Int(*i),
        Bson::Binary(bin) => schema::Value::Bytes(bin.bytes.clone()),
        // BSON-specific types: convert to string representation for display/error messages
        // (actual validation of these types happens in MongoDB-specific code)
        Bson::ObjectId(oid) => schema::Value::String(oid.to_hex()),
        Bson::DateTime(dt) => schema::Value::String(dt.to_string()),
        Bson::Decimal128(d) => schema::Value::String(d.to_string()),
        Bson::Timestamp(ts) => schema::Value::String(format!("{:?}", ts)),
        Bson::Symbol(s) => schema::Value::String(s.to_string()),
        _ => schema::Value::String(bson.to_string()),
    }
}

// =====================
// MongoDB-Specific Constraint Wrappers
// =====================

/// MongoDB-specific constraints wrapper
///
/// This maintains compatibility with the existing MongoDB-specific BsonTypeDescriptor API
/// while using cclab-shield for common validation logic.
#[derive(Debug, Clone, Default)]
pub struct BsonConstraints {
    /// String constraints from cclab-shield
    pub string: Option<schema::StringConstraints>,
    /// Numeric constraints from cclab-shield (f64)
    pub numeric: Option<schema::NumericConstraints<f64>>,
}

impl BsonConstraints {
    /// Check if any constraints are set
    pub fn is_empty(&self) -> bool {
        self.string.is_none() && self.numeric.is_none()
    }
}

// Manual PartialEq implementation to avoid rkyv conflicts
impl PartialEq for BsonConstraints {
    fn eq(&self, other: &Self) -> bool {
        // StringConstraints doesn't implement PartialEq, so we can't compare directly
        // For now, just compare if both are Some or None
        self.string.is_some() == other.string.is_some()
            && self.numeric.is_some() == other.numeric.is_some()
    }
}

// =====================
// Type Validation
// =====================

/// BSON type descriptor from Python type annotations
///
/// Represents the expected type of a field for validation purposes.
/// Each variant can optionally include constraints for field-level validation.
#[derive(Debug, Clone, PartialEq)]
pub enum BsonTypeDescriptor {
    /// String type with optional length and format constraints
    String { constraints: BsonConstraints },
    /// 64-bit integer with optional min/max constraints
    Int64 { constraints: BsonConstraints },
    /// Double-precision float with optional min/max constraints
    Double { constraints: BsonConstraints },
    /// Boolean
    Bool,
    /// Null
    Null,
    /// Binary data
    Binary,
    /// DateTime
    DateTime,
    /// Decimal128 with optional min/max constraints
    Decimal128 { constraints: BsonConstraints },
    /// ObjectId
    ObjectId,
    /// Array with items of a specific type
    Array { items: Box<BsonTypeDescriptor> },
    /// Object/document with a schema
    Object {
        schema: HashMap<String, BsonTypeDescriptor>,
    },
    /// Optional type (value can be null or the inner type)
    Optional { inner: Box<BsonTypeDescriptor> },
    /// Any type (no validation)
    Any,
}

impl BsonTypeDescriptor {
    /// Get the human-readable type name
    pub fn type_name(&self) -> String {
        match self {
            BsonTypeDescriptor::String { .. } => "string".to_string(),
            BsonTypeDescriptor::Int64 { .. } => "int64".to_string(),
            BsonTypeDescriptor::Double { .. } => "double".to_string(),
            BsonTypeDescriptor::Bool => "bool".to_string(),
            BsonTypeDescriptor::Null => "null".to_string(),
            BsonTypeDescriptor::Binary => "binary".to_string(),
            BsonTypeDescriptor::DateTime => "datetime".to_string(),
            BsonTypeDescriptor::Decimal128 { .. } => "decimal128".to_string(),
            BsonTypeDescriptor::ObjectId => "objectid".to_string(),
            BsonTypeDescriptor::Array { items } => format!("array<{}>", items.type_name()),
            BsonTypeDescriptor::Object { .. } => "object".to_string(),
            BsonTypeDescriptor::Optional { inner } => format!("optional<{}>", inner.type_name()),
            BsonTypeDescriptor::Any => "any".to_string(),
        }
    }

    /// Convert to schema::TypeDescriptor for common validation
    ///
    /// This enables using the unified validation engine while preserving MongoDB-specific types.
    /// BSON-specific types (ObjectId, DateTime, etc.) are returned as None since they need
    /// MongoDB-specific validation.
    pub fn to_validation_type_desc(&self) -> Option<schema::TypeDescriptor> {
        match self {
            BsonTypeDescriptor::String { constraints } => constraints
                .string
                .as_ref()
                .map(|c| schema::TypeDescriptor::String(c.clone())),
            BsonTypeDescriptor::Int64 { constraints } => {
                constraints.numeric.as_ref().map(|c| {
                    // Convert f64 constraints to i64
                    let i64_constraints = schema::NumericConstraints {
                        minimum: c.minimum.map(|v| v as i64),
                        maximum: c.maximum.map(|v| v as i64),
                        exclusive_minimum: c.exclusive_minimum.map(|v| v as i64),
                        exclusive_maximum: c.exclusive_maximum.map(|v| v as i64),
                        multiple_of: c.multiple_of.map(|v| v as i64),
                    };
                    schema::TypeDescriptor::Int64(i64_constraints)
                })
            }
            BsonTypeDescriptor::Double { constraints } => constraints
                .numeric
                .as_ref()
                .map(|c| schema::TypeDescriptor::Float64(c.clone())),
            BsonTypeDescriptor::Bool => Some(schema::TypeDescriptor::Bool),
            BsonTypeDescriptor::Null => Some(schema::TypeDescriptor::Null),
            BsonTypeDescriptor::Binary => Some(schema::TypeDescriptor::Bytes),
            BsonTypeDescriptor::Any => Some(schema::TypeDescriptor::Any),
            BsonTypeDescriptor::Optional { inner } => inner
                .to_validation_type_desc()
                .map(|inner_desc| schema::TypeDescriptor::Optional(Box::new(inner_desc))),
            // BSON-specific types need MongoDB-specific validation
            BsonTypeDescriptor::DateTime
            | BsonTypeDescriptor::Decimal128 { .. }
            | BsonTypeDescriptor::ObjectId
            | BsonTypeDescriptor::Array { .. }
            | BsonTypeDescriptor::Object { .. } => None,
        }
    }
}

/// Get the BSON type name for error messages
fn bson_type_name(value: &Bson) -> &'static str {
    match value {
        Bson::Double(_) => "double",
        Bson::String(_) => "string",
        Bson::Array(_) => "array",
        Bson::Document(_) => "object",
        Bson::Boolean(_) => "bool",
        Bson::Null => "null",
        Bson::Int32(_) => "int32",
        Bson::Int64(_) => "int64",
        Bson::Timestamp(_) => "timestamp",
        Bson::Binary(_) => "binary",
        Bson::ObjectId(_) => "objectid",
        Bson::DateTime(_) => "datetime",
        Bson::Symbol(_) => "symbol",
        Bson::Decimal128(_) => "decimal128",
        _ => "unknown",
    }
}

/// Validate a field value against its expected type
///
/// # Arguments
/// * `field_path` - Dot-notation path to the field (e.g., "address.city")
/// * `value` - The BSON value to validate
/// * `expected` - The expected type descriptor
///
/// # Errors
/// Returns ValidationError if the value doesn't match the expected type
pub fn validate_field(field_path: &str, value: &Bson, expected: &BsonTypeDescriptor) -> Result<()> {
    match expected {
        BsonTypeDescriptor::Any => Ok(()), // Any type is always valid

        BsonTypeDescriptor::Optional { inner } => {
            // Optional fields can be Null or match the inner type
            if matches!(value, Bson::Null) {
                Ok(())
            } else {
                validate_field(field_path, value, inner)
            }
        }

        BsonTypeDescriptor::String { constraints } => match value {
            Bson::String(s) => {
                // Use cclab-shield for common string validation
                if let Some(ref string_constraints) = constraints.string {
                    let validation_value = schema::Value::String(s.clone());
                    let type_desc =
                        schema::TypeDescriptor::String(string_constraints.clone());
                    let mut ctx = schema::ValidationContext::with_location(field_path);
                    let mut errors = schema::ValidationErrors::new();

                    schema::validate_value(
                        &validation_value,
                        &type_desc,
                        &mut ctx,
                        &mut errors,
                    );

                    if !errors.is_empty() {
                        return Err(DataBridgeError::Validation(errors.to_string()));
                    }
                }
                Ok(())
            }
            _ => Err(DataBridgeError::Validation(format!(
                "ValidationError: field '{}' expected type 'string', got '{}'",
                field_path,
                bson_type_name(value)
            ))),
        },

        BsonTypeDescriptor::Int64 { constraints } => match value {
            Bson::Int64(n) => {
                // Use cclab-shield for common numeric validation
                if let Some(ref numeric_constraints) = constraints.numeric {
                    let validation_value = schema::Value::Int(*n);
                    // Convert f64 constraints to i64 for proper type checking
                    let i64_constraints = schema::NumericConstraints {
                        minimum: numeric_constraints.minimum.map(|v| v as i64),
                        maximum: numeric_constraints.maximum.map(|v| v as i64),
                        exclusive_minimum: numeric_constraints.exclusive_minimum.map(|v| v as i64),
                        exclusive_maximum: numeric_constraints.exclusive_maximum.map(|v| v as i64),
                        multiple_of: numeric_constraints.multiple_of.map(|v| v as i64),
                    };
                    let type_desc = schema::TypeDescriptor::Int64(i64_constraints);
                    let mut ctx = schema::ValidationContext::with_location(field_path);
                    let mut errors = schema::ValidationErrors::new();

                    schema::validate_value(
                        &validation_value,
                        &type_desc,
                        &mut ctx,
                        &mut errors,
                    );

                    if !errors.is_empty() {
                        return Err(DataBridgeError::Validation(errors.to_string()));
                    }
                }
                Ok(())
            }
            Bson::Int32(n) => {
                // Convert Int32 to Int64 and validate
                if let Some(ref numeric_constraints) = constraints.numeric {
                    let validation_value = schema::Value::Int(*n as i64);
                    let i64_constraints = schema::NumericConstraints {
                        minimum: numeric_constraints.minimum.map(|v| v as i64),
                        maximum: numeric_constraints.maximum.map(|v| v as i64),
                        exclusive_minimum: numeric_constraints.exclusive_minimum.map(|v| v as i64),
                        exclusive_maximum: numeric_constraints.exclusive_maximum.map(|v| v as i64),
                        multiple_of: numeric_constraints.multiple_of.map(|v| v as i64),
                    };
                    let type_desc = schema::TypeDescriptor::Int64(i64_constraints);
                    let mut ctx = schema::ValidationContext::with_location(field_path);
                    let mut errors = schema::ValidationErrors::new();

                    schema::validate_value(
                        &validation_value,
                        &type_desc,
                        &mut ctx,
                        &mut errors,
                    );

                    if !errors.is_empty() {
                        return Err(DataBridgeError::Validation(errors.to_string()));
                    }
                }
                Ok(())
            }
            _ => Err(DataBridgeError::Validation(format!(
                "ValidationError: field '{}' expected type 'int64', got '{}'",
                field_path,
                bson_type_name(value)
            ))),
        },

        BsonTypeDescriptor::Double { constraints } => match value {
            Bson::Double(n) => {
                // Use cclab-shield for common numeric validation
                if let Some(ref numeric_constraints) = constraints.numeric {
                    let validation_value = schema::Value::Float(*n);
                    let type_desc =
                        schema::TypeDescriptor::Float64(numeric_constraints.clone());
                    let mut ctx = schema::ValidationContext::with_location(field_path);
                    let mut errors = schema::ValidationErrors::new();

                    schema::validate_value(
                        &validation_value,
                        &type_desc,
                        &mut ctx,
                        &mut errors,
                    );

                    if !errors.is_empty() {
                        return Err(DataBridgeError::Validation(errors.to_string()));
                    }
                }
                Ok(())
            }
            _ => Err(DataBridgeError::Validation(format!(
                "ValidationError: field '{}' expected type 'double', got '{}'",
                field_path,
                bson_type_name(value)
            ))),
        },

        BsonTypeDescriptor::Bool => match value {
            Bson::Boolean(_) => Ok(()),
            _ => Err(DataBridgeError::Validation(format!(
                "ValidationError: field '{}' expected type 'bool', got '{}'",
                field_path,
                bson_type_name(value)
            ))),
        },

        BsonTypeDescriptor::Null => match value {
            Bson::Null => Ok(()),
            _ => Err(DataBridgeError::Validation(format!(
                "ValidationError: field '{}' expected type 'null', got '{}'",
                field_path,
                bson_type_name(value)
            ))),
        },

        BsonTypeDescriptor::Binary => match value {
            Bson::Binary(_) => Ok(()),
            _ => Err(DataBridgeError::Validation(format!(
                "ValidationError: field '{}' expected type 'binary', got '{}'",
                field_path,
                bson_type_name(value)
            ))),
        },

        BsonTypeDescriptor::DateTime => match value {
            Bson::DateTime(_) => Ok(()),
            _ => Err(DataBridgeError::Validation(format!(
                "ValidationError: field '{}' expected type 'datetime', got '{}'",
                field_path,
                bson_type_name(value)
            ))),
        },

        BsonTypeDescriptor::Decimal128 { constraints } => match value {
            Bson::Decimal128(d) => {
                // Use cclab-shield for common numeric validation
                // Convert Decimal128 to f64 for constraint validation
                // Note: This may lose precision for very large/small values
                if let Some(ref numeric_constraints) = constraints.numeric {
                    let d_str = d.to_string();
                    if let Ok(n) = d_str.parse::<f64>() {
                        let validation_value = schema::Value::Float(n);
                        let type_desc =
                            schema::TypeDescriptor::Float64(numeric_constraints.clone());
                        let mut ctx = schema::ValidationContext::with_location(field_path);
                        let mut errors = schema::ValidationErrors::new();

                        schema::validate_value(
                            &validation_value,
                            &type_desc,
                            &mut ctx,
                            &mut errors,
                        );

                        if !errors.is_empty() {
                            return Err(DataBridgeError::Validation(errors.to_string()));
                        }
                    }
                }
                Ok(())
            }
            _ => Err(DataBridgeError::Validation(format!(
                "ValidationError: field '{}' expected type 'decimal128', got '{}'",
                field_path,
                bson_type_name(value)
            ))),
        },

        BsonTypeDescriptor::ObjectId => match value {
            Bson::ObjectId(_) => Ok(()),
            _ => Err(DataBridgeError::Validation(format!(
                "ValidationError: field '{}' expected type 'objectid', got '{}'",
                field_path,
                bson_type_name(value)
            ))),
        },

        BsonTypeDescriptor::Array { items } => match value {
            Bson::Array(arr) => {
                // Validate each array element
                for (index, item) in arr.iter().enumerate() {
                    let item_path = format!("{}[{}]", field_path, index);
                    validate_field(&item_path, item, items)?;
                }
                Ok(())
            }
            _ => Err(DataBridgeError::Validation(format!(
                "ValidationError: field '{}' expected type 'array', got '{}'",
                field_path,
                bson_type_name(value)
            ))),
        },

        BsonTypeDescriptor::Object { schema } => match value {
            Bson::Document(doc) => {
                // Validate each field in the document
                for (field_name, field_type) in schema {
                    if let Some(field_value) = doc.get(field_name) {
                        let nested_path = format!("{}.{}", field_path, field_name);
                        validate_field(&nested_path, field_value, field_type)?;
                    }
                }
                Ok(())
            }
            _ => Err(DataBridgeError::Validation(format!(
                "ValidationError: field '{}' expected type 'object', got '{}'",
                field_path,
                bson_type_name(value)
            ))),
        },
    }
}

/// Validate an entire document against a schema
///
/// # Arguments
/// * `data` - The BSON document to validate
/// * `schema` - Map of field names to their expected types
///
/// # Errors
/// Returns ValidationError on the first validation error encountered
pub fn validate_document(
    data: &bson::Document,
    schema: &HashMap<String, BsonTypeDescriptor>,
) -> Result<()> {
    for (field_name, field_type) in schema {
        if let Some(field_value) = data.get(field_name) {
            validate_field(field_name, field_value, field_type)?;
        }
        // Note: Missing fields are allowed (they may be optional or have defaults)
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bson::doc;

    // =====================
    // ValidatedCollectionName Tests
    // =====================

    #[test]
    fn test_valid_collection_names() {
        let names = vec!["users", "posts", "my_collection", "test123"];
        for name in names {
            assert!(
                ValidatedCollectionName::new(name).is_ok(),
                "Should accept valid name: {}",
                name
            );
        }
    }

    #[test]
    fn test_empty_collection_name() {
        let result = ValidatedCollectionName::new("");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("empty"));
    }

    #[test]
    fn test_collection_name_too_long() {
        let long_name = "a".repeat(MAX_COLLECTION_NAME_LENGTH + 1);
        let result = ValidatedCollectionName::new(&long_name);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("maximum length"));
    }

    #[test]
    fn test_collection_name_with_null_byte() {
        let result = ValidatedCollectionName::new("test\0collection");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("null bytes"));
    }

    #[test]
    fn test_system_collection_blocked() {
        let result = ValidatedCollectionName::new("system.users");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("system."));
    }

    #[test]
    fn test_collection_name_with_dollar_sign() {
        let result = ValidatedCollectionName::new("$users");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("$"));
    }

    #[test]
    fn test_validated_collection_name_display() {
        let validated = ValidatedCollectionName::new("users").unwrap();
        assert_eq!(validated.as_str(), "users");
        assert_eq!(validated.to_string(), "users");
    }

    // =====================
    // ValidatedFieldName Tests
    // =====================

    #[test]
    fn test_valid_field_names() {
        let names = vec!["email", "user_id", "created_at", "nested.field"];
        for name in names {
            assert!(
                ValidatedFieldName::new(name, false).is_ok(),
                "Should accept valid field name: {}",
                name
            );
        }
    }

    #[test]
    fn test_empty_field_name() {
        let result = ValidatedFieldName::new("", false);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("empty"));
    }

    #[test]
    fn test_field_name_too_long() {
        let long_name = "a".repeat(MAX_FIELD_NAME_LENGTH + 1);
        let result = ValidatedFieldName::new(&long_name, false);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("maximum length"));
    }

    #[test]
    fn test_field_name_with_null_byte() {
        let result = ValidatedFieldName::new("test\0field", false);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("null bytes"));
    }

    #[test]
    fn test_field_name_with_dollar_sign_not_allowed() {
        let result = ValidatedFieldName::new("$set", false);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("$"));
    }

    #[test]
    fn test_field_name_with_dollar_sign_allowed() {
        let result = ValidatedFieldName::new("$set", true);
        assert!(result.is_ok());
    }

    #[test]
    fn test_known_update_operators() {
        let operators = vec!["$set", "$unset", "$inc", "$push"];
        for op in operators {
            assert!(
                ValidatedFieldName::new(op, true).is_ok(),
                "Should accept known operator: {}",
                op
            );
        }
    }

    // =====================
    // ObjectIdParser Tests
    // =====================

    #[test]
    fn test_objectid_parser_with_type_hint() {
        let valid_oid = "507f1f77bcf86cd799439011";
        assert!(ObjectIdParser::should_convert_to_objectid(
            valid_oid,
            Some("ObjectId")
        ));
        assert!(ObjectIdParser::should_convert_to_objectid(
            valid_oid,
            Some("PydanticObjectId")
        ));
    }

    #[test]
    fn test_objectid_parser_without_type_hint() {
        let valid_oid = "507f1f77bcf86cd799439011";
        // Without type hint, should NOT convert (security!)
        assert!(!ObjectIdParser::should_convert_to_objectid(valid_oid, None));
    }

    #[test]
    fn test_objectid_parser_invalid_format() {
        let invalid = "not-an-objectid";
        assert!(!ObjectIdParser::should_convert_to_objectid(
            invalid,
            Some("ObjectId")
        ));
    }

    #[test]
    fn test_objectid_parser_all_zeros_attack() {
        let all_zeros = "000000000000000000000000";
        // Without type hint, should NOT convert (prevents injection!)
        assert!(!ObjectIdParser::should_convert_to_objectid(all_zeros, None));
        // Even with type hint, format is valid
        assert!(ObjectIdParser::should_convert_to_objectid(
            all_zeros,
            Some("ObjectId")
        ));
    }

    // =====================
    // Query Validation Tests
    // =====================

    #[test]
    fn test_validate_safe_query() {
        let safe = doc! {"email": "test@example.com", "age": 25};
        assert!(validate_query(&bson::Bson::Document(safe)).is_ok());
    }

    #[test]
    fn test_validate_query_with_where_operator() {
        let dangerous = doc! {"$where": "this.email == 'admin@example.com'"};
        let result = validate_query(&bson::Bson::Document(dangerous));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("$where"));
    }

    #[test]
    fn test_validate_query_with_function_operator() {
        let dangerous = doc! {
            "$function": {
                "body": "function() { return true; }",
                "args": [],
                "lang": "js"
            }
        };
        let result = validate_query(&bson::Bson::Document(dangerous));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("$function"));
    }

    #[test]
    fn test_validate_nested_query() {
        let nested = doc! {
            "$and": [
                {"email": "test@example.com"},
                {"$where": "this.age > 18"}
            ]
        };
        let result = validate_query(&bson::Bson::Document(nested));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("$where"));
    }

    #[test]
    fn test_validate_safe_nested_query() {
        let safe = doc! {
            "$and": [
                {"email": "test@example.com"},
                {"age": {"$gt": 18}}
            ]
        };
        assert!(validate_query(&bson::Bson::Document(safe)).is_ok());
    }
}
