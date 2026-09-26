//! Cclab Validation
//!
//! Unified validation library for the Cclab framework.
//!
//! This crate provides Pydantic-like validation with Rust performance,
//! serving as the validation foundation for:
//! - `cclab-quasar`: HTTP request validation
//! - `cclab`: MongoDB/BSON validation
//! - `cclab-titan`: PostgreSQL identifier validation
//! - `cclab-sheet-*`: Spreadsheet validation
//!
//! # Architecture Vision
//!
//! ```text
//! cclab.orbit      == uvloop             (event loop)
//! cclab.quasar         == uvicorn + fastapi  (web framework)
//! cclab.shield  == pydantic + orjson  (validation + JSON) ⭐
//! ```
//!
//! # Features
//!
//! - **Default**: Core validation without serialization
//! - **serde**: JSON serialization with `serde_json`
//! - **sonic**: High-performance JSON with `sonic-rs` (3-7x faster)
//! - **bson**: MongoDB BSON type support
//!
//! # Example
//!
//! ```rust
//! use schema::{TypeDescriptor, Value, validate};
//! use schema::constraints::StringConstraints;
//!
//! // Define type
//! let email_type = TypeDescriptor::Email;
//!
//! // Validate value
//! let value = Value::String("user@example.com".to_string());
//! let result = validate(&value, &email_type);
//! assert!(result.is_ok());
//!
//! // Invalid email
//! let invalid = Value::String("not-an-email".to_string());
//! let result = validate(&invalid, &email_type);
//! assert!(result.is_err());
//! ```

// Public modules
pub mod computed;
pub mod constraints;
pub mod custom_validators;
pub mod errors;
pub mod formats;
pub mod serializers;
pub mod types;
pub mod validators;

// Additional validation features
pub mod coercion;
pub mod config;
pub mod dataclass;
pub mod discriminated;
pub mod generics;
pub mod json_schema;
pub mod protobuf;
pub mod settings;
pub mod settings_source;
pub mod strict;

// Re-export error module under 'error' alias for compatibility
pub mod error {
    pub use crate::errors::*;
}

// Python bindings (feature-gated)

// Re-export commonly used types
pub use computed::{
    get_bool_field, get_float_field, get_int_field, get_string_field, BoxedComputedField,
    ComputedField, ComputedFieldCollection, ConcatComputed, FnComputedField,
};
pub use constraints::{
    FieldDescriptor, ListConstraints, NumericConstraints, StringConstraints, StringFormat,
};
pub use custom_validators::{
    custom_error, field_error, BoxedFieldValidator, BoxedModelValidator, FieldValidator,
    FnFieldValidator, FnModelValidator, ModelValidator, ValidatorCollection, ValidatorContext,
    ValidatorMode,
};
pub use errors::{
    ErrorType, ValidationContext, ValidationError, ValidationErrors, ValidationResult,
};
pub use serializers::{
    BoxedFieldSerializer, BoxedModelSerializer, FieldSerializer, FnFieldSerializer,
    FnModelSerializer, MaskSerializer, ModelSerializer, SerializerCollection, SerializerContext,
    SerializerMode,
};
pub use types::{RootModel, TypeDescriptor, Value};
pub use validators::{validate, validate_value, validate_with_context};

// Additional feature re-exports
pub use coercion::{
    apply_coercion, coerce_value, CoercionMode, CoercionResult, MAX_COERCION_DEPTH,
};
pub use config::{ExtraFields, RevalidateInstances, ValidationConfig};
pub use dataclass::{infer_type_from_annotation, DataclassDefinition, FieldInfo};
pub use discriminated::{DiscriminatedUnion, DiscriminatedUnionBuilder};
pub use generics::{
    substitute_type_params, GenericError, GenericTypeBuilder, GenericTypeDef, TypeParam,
    TypeParamBound,
};
pub use json_schema::{type_descriptor_to_json_schema, JsonSchema};
pub use protobuf::{generate_proto_file, type_descriptor_to_proto, ProtoOptions};
pub use strict::{StrictMode, StrictResult};

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
    }
}
