//! ORM layer — declarative models, query builder, validation, back-refs
//! (SQLAlchemy equivalent).
//!
//! Built on top of `driver`. May depend on `driver`; MUST NOT depend on
//! `migrate`.

pub mod auto_detect;
pub mod backref;
pub mod compat;
pub mod query;
pub mod schema;
pub mod session;
pub mod validation;

pub use auto_detect::{
    AutoDetectConfig, AutoDetectResult, AutoDetector, ForeignKeyRef, ModelDefinition, ModelField,
    ModelIndex,
};
pub use backref::{BackRefConfig, BackRefLoader, EagerLoader, EagerRelation};
pub use compat::{
    ComputedFieldConfig, EmailValidator, FieldValidatorConfig, LengthValidator,
    ModelValidatorConfig, PatternValidator, RangeValidator, UrlValidator, ValidationError,
    ValidationErrors, ValidationMode, ValidationRegistry,
};
pub use query::{
    AggregateFunction, HavingCondition, JoinCondition, JoinType, Operator, OrderDirection,
    QueryBuilder, WindowExpression, WindowFunction, WindowSpec,
};
pub use schema::{BackRef, CascadeRule, ManyToManyConfig, SchemaInspector};
pub use session::{Session, SessionModel, SessionQuery};
pub use validation::validate_foreign_key_reference;

/// Re-export aggregator for the blocking ORM surface.
///
/// `pg::orm::blocking::Session` resolves to the orm blocking
/// Session. The crate-root `crate::blocking` aggregator pulls both
/// driver and orm blocking surfaces together.
pub mod blocking {
    pub use crate::orm::session::blocking::{Session, SessionQuery};
}
