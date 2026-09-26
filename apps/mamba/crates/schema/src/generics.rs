//! Generic type support for cclab-shield
//!
//! Provides Generic[T] functionality similar to Pydantic v2, enabling
//! reusable type templates with type parameters.
//!
//! # Example
//!
//! ```rust,ignore
//! use schema::generics::{GenericTypeDef, TypeParam};
//! use schema::types::TypeDescriptor;
//! use schema::constraints::FieldDescriptor;
//!
//! // Define a generic Response[T] type
//! let response_def = GenericTypeDef::new("Response")
//!     .type_param("T")
//!     .template(TypeDescriptor::Object {
//!         fields: vec![
//!             FieldDescriptor::new("data", TypeDescriptor::TypeParam("T".into())),
//!             FieldDescriptor::new("status", TypeDescriptor::Int64(Default::default())),
//!         ],
//!         additional: None,
//!     });
//!
//! // Instantiate as Response[User]
//! let user_response = response_def.instantiate(&[user_type])?;
//! ```

use std::collections::HashMap;

use crate::constraints::FieldDescriptor;
use crate::types::TypeDescriptor;

// ============================================================================
// Type Parameter
// ============================================================================

/// A type parameter placeholder (e.g., T, K, V)
#[derive(Debug, Clone)]
pub struct TypeParam {
    /// Parameter name (e.g., "T", "K", "V")
    pub name: String,
    /// Optional bound/constraint on the type parameter
    pub bound: Option<TypeParamBound>,
}

impl PartialEq for TypeParam {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for TypeParam {}

impl std::hash::Hash for TypeParam {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

impl TypeParam {
    /// Create a new type parameter
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            bound: None,
        }
    }

    /// Create a type parameter with a bound
    pub fn with_bound(name: impl Into<String>, bound: TypeParamBound) -> Self {
        Self {
            name: name.into(),
            bound: Some(bound),
        }
    }
}

impl From<&str> for TypeParam {
    fn from(name: &str) -> Self {
        Self::new(name)
    }
}

impl From<String> for TypeParam {
    fn from(name: String) -> Self {
        Self::new(name)
    }
}

// ============================================================================
// Type Parameter Bounds
// ============================================================================

/// Constraints on what types can be used for a type parameter
#[derive(Debug, Clone)]
pub enum TypeParamBound {
    /// Must be one of these specific types
    OneOf(Vec<TypeDescriptor>),
    /// Must be a subtype (for objects, must have at least these fields)
    Extends(Box<TypeDescriptor>),
    /// No constraint (accepts any type)
    Any,
}

impl Default for TypeParamBound {
    fn default() -> Self {
        Self::Any
    }
}

// ============================================================================
// Generic Type Definition
// ============================================================================

/// A generic type definition with type parameters
///
/// Similar to Python's `class MyType(Generic[T, K])` pattern.
#[derive(Debug, Clone)]
pub struct GenericTypeDef {
    /// Name of the generic type (e.g., "Response", "Page")
    pub name: String,
    /// Type parameters in order (e.g., ["T"], ["K", "V"])
    pub type_params: Vec<TypeParam>,
    /// The type template with TypeParam placeholders
    pub template: TypeDescriptor,
}

impl GenericTypeDef {
    /// Create a new generic type definition
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            type_params: Vec::new(),
            template: TypeDescriptor::Any,
        }
    }

    /// Add a type parameter
    pub fn type_param(mut self, name: impl Into<String>) -> Self {
        self.type_params.push(TypeParam::new(name));
        self
    }

    /// Add a type parameter with a bound
    pub fn type_param_bounded(mut self, name: impl Into<String>, bound: TypeParamBound) -> Self {
        self.type_params.push(TypeParam::with_bound(name, bound));
        self
    }

    /// Set the type template
    pub fn template(mut self, template: TypeDescriptor) -> Self {
        self.template = template;
        self
    }

    /// Create an Object template with fields (convenience method)
    pub fn fields(mut self, fields: Vec<FieldDescriptor>) -> Self {
        self.template = TypeDescriptor::Object {
            fields,
            additional: None,
        };
        self
    }

    /// Get the number of type parameters
    pub fn arity(&self) -> usize {
        self.type_params.len()
    }

    /// Instantiate the generic type with concrete type arguments
    ///
    /// # Arguments
    /// * `args` - Concrete types to substitute for type parameters (in order)
    ///
    /// # Returns
    /// A fully resolved TypeDescriptor with all type parameters replaced
    ///
    /// # Errors
    /// Returns an error if the number of arguments doesn't match the parameters
    pub fn instantiate(&self, args: &[TypeDescriptor]) -> Result<TypeDescriptor, GenericError> {
        if args.len() != self.type_params.len() {
            return Err(GenericError::ArityMismatch {
                expected: self.type_params.len(),
                got: args.len(),
                type_name: self.name.clone(),
            });
        }

        // Build substitution map
        let mut substitutions = HashMap::new();
        for (param, arg) in self.type_params.iter().zip(args.iter()) {
            // Check bounds if present
            if let Some(ref bound) = param.bound {
                if !check_bound(arg, bound) {
                    return Err(GenericError::BoundViolation {
                        param: param.name.clone(),
                        type_name: self.name.clone(),
                    });
                }
            }
            substitutions.insert(param.name.clone(), arg.clone());
        }

        // Substitute type parameters in template
        Ok(substitute_type_params(&self.template, &substitutions))
    }

    /// Create a builder for instantiating this generic type
    pub fn builder(&self) -> GenericTypeBuilder<'_> {
        GenericTypeBuilder::new(self)
    }
}

// ============================================================================
// Generic Type Builder
// ============================================================================

/// Builder for instantiating a generic type with fluent API
pub struct GenericTypeBuilder<'a> {
    def: &'a GenericTypeDef,
    args: Vec<TypeDescriptor>,
}

impl<'a> GenericTypeBuilder<'a> {
    /// Create a new builder
    pub fn new(def: &'a GenericTypeDef) -> Self {
        Self {
            def,
            args: Vec::new(),
        }
    }

    /// Add a type argument
    pub fn arg(mut self, type_desc: TypeDescriptor) -> Self {
        self.args.push(type_desc);
        self
    }

    /// Build the instantiated type
    pub fn build(self) -> Result<TypeDescriptor, GenericError> {
        self.def.instantiate(&self.args)
    }
}

// ============================================================================
// Type Parameter Substitution
// ============================================================================

/// Recursively substitute type parameters in a TypeDescriptor
pub fn substitute_type_params(
    desc: &TypeDescriptor,
    substitutions: &HashMap<String, TypeDescriptor>,
) -> TypeDescriptor {
    match desc {
        // Type parameter placeholder - substitute it
        TypeDescriptor::TypeParam(name) => substitutions
            .get(name)
            .cloned()
            .unwrap_or_else(|| desc.clone()),

        // Recursively substitute in nested types
        TypeDescriptor::Optional(inner) => {
            TypeDescriptor::Optional(Box::new(substitute_type_params(inner, substitutions)))
        }

        TypeDescriptor::List { items, constraints } => TypeDescriptor::List {
            items: Box::new(substitute_type_params(items, substitutions)),
            constraints: constraints.clone(),
        },

        TypeDescriptor::Set { items } => TypeDescriptor::Set {
            items: Box::new(substitute_type_params(items, substitutions)),
        },

        TypeDescriptor::Tuple { items } => TypeDescriptor::Tuple {
            items: items
                .iter()
                .map(|item| substitute_type_params(item, substitutions))
                .collect(),
        },

        TypeDescriptor::Object { fields, additional } => TypeDescriptor::Object {
            fields: fields
                .iter()
                .map(|field| FieldDescriptor {
                    name: field.name.clone(),
                    type_desc: substitute_type_params(&field.type_desc, substitutions),
                    required: field.required,
                    default: field.default.clone(),
                    description: field.description.clone(),
                    title: field.title.clone(),
                    examples: field.examples.clone(),
                    alias: field.alias.clone(),
                    validation_alias: field.validation_alias.clone(),
                    serialization_alias: field.serialization_alias.clone(),
                    private: field.private,
                    init_only: field.init_only,
                    deprecated: field.deprecated,
                    read_only: field.read_only,
                    write_only: field.write_only,
                })
                .collect(),
            additional: additional
                .as_ref()
                .map(|t| Box::new(substitute_type_params(t, substitutions))),
        },

        TypeDescriptor::Union { variants, nullable } => TypeDescriptor::Union {
            variants: variants
                .iter()
                .map(|v| substitute_type_params(v, substitutions))
                .collect(),
            nullable: *nullable,
        },

        // Primitive and format types - no substitution needed
        _ => desc.clone(),
    }
}

/// Check if a type satisfies a bound
fn check_bound(type_desc: &TypeDescriptor, bound: &TypeParamBound) -> bool {
    match bound {
        TypeParamBound::Any => true,
        TypeParamBound::OneOf(allowed) => allowed.iter().any(|t| types_compatible(type_desc, t)),
        TypeParamBound::Extends(base) => is_subtype(type_desc, base),
    }
}

/// Check if two types are compatible (same structure)
fn types_compatible(a: &TypeDescriptor, b: &TypeDescriptor) -> bool {
    match (a, b) {
        (TypeDescriptor::Any, _) | (_, TypeDescriptor::Any) => true,
        (TypeDescriptor::String(_), TypeDescriptor::String(_)) => true,
        (TypeDescriptor::Int64(_), TypeDescriptor::Int64(_)) => true,
        (TypeDescriptor::Float64(_), TypeDescriptor::Float64(_)) => true,
        (TypeDescriptor::Bool, TypeDescriptor::Bool) => true,
        (TypeDescriptor::List { items: a, .. }, TypeDescriptor::List { items: b, .. }) => {
            types_compatible(a, b)
        }
        (TypeDescriptor::Object { .. }, TypeDescriptor::Object { .. }) => true,
        _ => std::mem::discriminant(a) == std::mem::discriminant(b),
    }
}

/// Check if a type is a subtype of another (for Extends bound)
fn is_subtype(sub: &TypeDescriptor, base: &TypeDescriptor) -> bool {
    match (sub, base) {
        (_, TypeDescriptor::Any) => true,
        (
            TypeDescriptor::Object {
                fields: sub_fields, ..
            },
            TypeDescriptor::Object {
                fields: base_fields,
                ..
            },
        ) => {
            // Check that sub has all fields from base
            base_fields.iter().all(|base_field| {
                sub_fields.iter().any(|sub_field| {
                    sub_field.name == base_field.name
                        && types_compatible(&sub_field.type_desc, &base_field.type_desc)
                })
            })
        }
        _ => types_compatible(sub, base),
    }
}

// ============================================================================
// Errors
// ============================================================================

/// Errors that can occur when working with generic types
#[derive(Debug, Clone, PartialEq)]
pub enum GenericError {
    /// Wrong number of type arguments provided
    ArityMismatch {
        expected: usize,
        got: usize,
        type_name: String,
    },
    /// Type argument doesn't satisfy parameter bound
    BoundViolation { param: String, type_name: String },
    /// Unresolved type parameter (substitution failed)
    UnresolvedTypeParam { param: String },
}

impl std::fmt::Display for GenericError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ArityMismatch {
                expected,
                got,
                type_name,
            } => {
                write!(
                    f,
                    "Generic type '{}' expects {} type argument(s), got {}",
                    type_name, expected, got
                )
            }
            Self::BoundViolation { param, type_name } => {
                write!(
                    f,
                    "Type argument for '{}' in '{}' violates parameter bound",
                    param, type_name
                )
            }
            Self::UnresolvedTypeParam { param } => {
                write!(f, "Unresolved type parameter: {}", param)
            }
        }
    }
}

impl std::error::Error for GenericError {}

// ============================================================================
// Common Generic Types (Prebuilt)
// ============================================================================

/// Predefined generic types for common patterns
pub mod prelude {
    use super::*;
    use crate::constraints::FieldDescriptor;

    /// Response[T] - API response wrapper
    ///
    /// ```rust,ignore
    /// Response[T] = { data: T, success: bool, message?: string }
    /// ```
    pub fn response_type() -> GenericTypeDef {
        GenericTypeDef::new("Response").type_param("T").fields(vec![
            FieldDescriptor::new("data", TypeDescriptor::TypeParam("T".into())),
            FieldDescriptor::new("success", TypeDescriptor::Bool),
            FieldDescriptor::new("message", TypeDescriptor::String(Default::default())).optional(),
        ])
    }

    /// Page[T] - Paginated list wrapper
    ///
    /// ```rust,ignore
    /// Page[T] = { items: List[T], total: int, page: int, page_size: int }
    /// ```
    pub fn page_type() -> GenericTypeDef {
        GenericTypeDef::new("Page").type_param("T").fields(vec![
            FieldDescriptor::new(
                "items",
                TypeDescriptor::List {
                    items: Box::new(TypeDescriptor::TypeParam("T".into())),
                    constraints: Default::default(),
                },
            ),
            FieldDescriptor::new("total", TypeDescriptor::Int64(Default::default())),
            FieldDescriptor::new("page", TypeDescriptor::Int64(Default::default())),
            FieldDescriptor::new("page_size", TypeDescriptor::Int64(Default::default())),
        ])
    }

    /// Result[T, E] - Result type with success and error variants
    ///
    /// ```rust,ignore
    /// Result[T, E] = { ok: bool, value?: T, error?: E }
    /// ```
    pub fn result_type() -> GenericTypeDef {
        GenericTypeDef::new("Result")
            .type_param("T")
            .type_param("E")
            .fields(vec![
                FieldDescriptor::new("ok", TypeDescriptor::Bool),
                FieldDescriptor::new("value", TypeDescriptor::TypeParam("T".into())).optional(),
                FieldDescriptor::new("error", TypeDescriptor::TypeParam("E".into())).optional(),
            ])
    }

    /// KeyValue[K, V] - Key-value pair
    ///
    /// ```rust,ignore
    /// KeyValue[K, V] = { key: K, value: V }
    /// ```
    pub fn key_value_type() -> GenericTypeDef {
        GenericTypeDef::new("KeyValue")
            .type_param("K")
            .type_param("V")
            .fields(vec![
                FieldDescriptor::new("key", TypeDescriptor::TypeParam("K".into())),
                FieldDescriptor::new("value", TypeDescriptor::TypeParam("V".into())),
            ])
    }

    /// Wrapper[T] - Simple value wrapper
    ///
    /// ```rust,ignore
    /// Wrapper[T] = { value: T }
    /// ```
    pub fn wrapper_type() -> GenericTypeDef {
        GenericTypeDef::new("Wrapper")
            .type_param("T")
            .fields(vec![FieldDescriptor::new(
                "value",
                TypeDescriptor::TypeParam("T".into()),
            )])
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
    fn test_simple_generic() {
        let wrapper =
            GenericTypeDef::new("Wrapper")
                .type_param("T")
                .fields(vec![FieldDescriptor::new(
                    "value",
                    TypeDescriptor::TypeParam("T".into()),
                )]);

        // Instantiate Wrapper[String]
        let string_wrapper = wrapper
            .instantiate(&[TypeDescriptor::String(Default::default())])
            .unwrap();

        let TypeDescriptor::Object { fields, .. } = string_wrapper else {
            panic!("Expected Object");
        };

        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].name, "value");
        assert!(matches!(fields[0].type_desc, TypeDescriptor::String(_)));
    }

    #[test]
    fn test_multi_param_generic() {
        let kv = GenericTypeDef::new("KeyValue")
            .type_param("K")
            .type_param("V")
            .fields(vec![
                FieldDescriptor::new("key", TypeDescriptor::TypeParam("K".into())),
                FieldDescriptor::new("value", TypeDescriptor::TypeParam("V".into())),
            ]);

        // Instantiate KeyValue[String, Int]
        let string_int_kv = kv
            .instantiate(&[
                TypeDescriptor::String(Default::default()),
                TypeDescriptor::Int64(Default::default()),
            ])
            .unwrap();

        let TypeDescriptor::Object { fields, .. } = string_int_kv else {
            panic!("Expected Object");
        };

        assert_eq!(fields.len(), 2);
        assert!(matches!(fields[0].type_desc, TypeDescriptor::String(_)));
        assert!(matches!(fields[1].type_desc, TypeDescriptor::Int64(_)));
    }

    #[test]
    fn test_nested_generic() {
        // Page[T] with items: List[T]
        let page = GenericTypeDef::new("Page")
            .type_param("T")
            .fields(vec![FieldDescriptor::new(
                "items",
                TypeDescriptor::List {
                    items: Box::new(TypeDescriptor::TypeParam("T".into())),
                    constraints: Default::default(),
                },
            )]);

        // Instantiate Page[String]
        let string_page = page
            .instantiate(&[TypeDescriptor::String(Default::default())])
            .unwrap();

        let TypeDescriptor::Object { fields, .. } = string_page else {
            panic!("Expected Object");
        };

        let TypeDescriptor::List { items, .. } = &fields[0].type_desc else {
            panic!("Expected List");
        };

        assert!(matches!(items.as_ref(), TypeDescriptor::String(_)));
    }

    #[test]
    fn test_arity_mismatch() {
        let wrapper = GenericTypeDef::new("Wrapper").type_param("T");

        let result = wrapper.instantiate(&[]);
        assert!(matches!(result, Err(GenericError::ArityMismatch { .. })));

        let result = wrapper.instantiate(&[
            TypeDescriptor::String(Default::default()),
            TypeDescriptor::Int64(Default::default()),
        ]);
        assert!(matches!(result, Err(GenericError::ArityMismatch { .. })));
    }

    #[test]
    fn test_builder_api() {
        let wrapper =
            GenericTypeDef::new("Wrapper")
                .type_param("T")
                .fields(vec![FieldDescriptor::new(
                    "value",
                    TypeDescriptor::TypeParam("T".into()),
                )]);

        let result = wrapper
            .builder()
            .arg(TypeDescriptor::Int64(Default::default()))
            .build()
            .unwrap();

        let TypeDescriptor::Object { fields, .. } = result else {
            panic!("Expected Object");
        };

        assert!(matches!(fields[0].type_desc, TypeDescriptor::Int64(_)));
    }

    #[test]
    fn test_prelude_response() {
        let response = prelude::response_type();
        assert_eq!(response.name, "Response");
        assert_eq!(response.arity(), 1);

        // Response[User]
        let user_type = TypeDescriptor::Object {
            fields: vec![
                FieldDescriptor::new("id", TypeDescriptor::Int64(Default::default())),
                FieldDescriptor::new("name", TypeDescriptor::String(Default::default())),
            ],
            additional: None,
        };

        let user_response = response.instantiate(&[user_type]).unwrap();

        let TypeDescriptor::Object { fields, .. } = user_response else {
            panic!("Expected Object");
        };

        assert_eq!(fields.len(), 3);
        assert_eq!(fields[0].name, "data");
        assert!(matches!(fields[0].type_desc, TypeDescriptor::Object { .. }));
    }

    #[test]
    fn test_prelude_page() {
        let page = prelude::page_type();
        assert_eq!(page.name, "Page");

        let user_page = page
            .instantiate(&[TypeDescriptor::String(Default::default())])
            .unwrap();

        let TypeDescriptor::Object { fields, .. } = user_page else {
            panic!("Expected Object");
        };

        assert_eq!(fields.len(), 4);
        assert_eq!(fields[0].name, "items");

        let TypeDescriptor::List { items, .. } = &fields[0].type_desc else {
            panic!("Expected List");
        };

        assert!(matches!(items.as_ref(), TypeDescriptor::String(_)));
    }

    #[test]
    fn test_type_param_bound() {
        // Generic constrained to only accept String or Int
        let bounded = GenericTypeDef::new("Bounded")
            .type_param_bounded(
                "T",
                TypeParamBound::OneOf(vec![
                    TypeDescriptor::String(Default::default()),
                    TypeDescriptor::Int64(Default::default()),
                ]),
            )
            .fields(vec![FieldDescriptor::new(
                "value",
                TypeDescriptor::TypeParam("T".into()),
            )]);

        // Valid: String
        assert!(bounded
            .instantiate(&[TypeDescriptor::String(Default::default())])
            .is_ok());

        // Valid: Int
        assert!(bounded
            .instantiate(&[TypeDescriptor::Int64(Default::default())])
            .is_ok());

        // Invalid: Bool
        assert!(matches!(
            bounded.instantiate(&[TypeDescriptor::Bool]),
            Err(GenericError::BoundViolation { .. })
        ));
    }
}
