//! Native PostgreSQL toolkit core for Mamba-facing applications.
//!
//! This crate owns the source/logic side of pgkit: PostgreSQL driver access,
//! ORM/query primitives, schema migration, validation, metrics, and the blocking
//! facade used by language interfaces. Mamba bindings live in the sibling
//! `projects/pgkit/mamba-binding` crate and expose this core as `mambalibs.pg`.
//!
//! - Zero Mamba heap pressure in SQL generation and serialization
//! - Native async I/O with a sync facade for binding layers
//! - Parallel processing for bulk operations
//! - Security-first design with input validation
//! - Copy-on-Write state management
//!
//! # Architecture
//!
//! ```text
//! Mamba source layer (models, queries, migrations)
//!           |
//!      Mamba interface layer (`mambalibs.pg`)
//!           |
//!    Native pgkit core (`projects/pgkit/pg/src`)
//!           |
//!         SQLx (PostgreSQL driver)
//! ```
//!
//! # Key Features
//!
//! - **Advanced Query Builder**: Fluent API with support for complex queries including:
//!   - Joins (INNER, LEFT, RIGHT, FULL OUTER)
//!   - Subqueries and CTEs (Common Table Expressions)
//!   - Window functions (ROW_NUMBER, RANK, LEAD, LAG)
//!   - Aggregations with GROUP BY and HAVING clauses
//!   - DISTINCT ON for PostgreSQL-specific deduplication
//!
//! - **Relationship Management**:
//!   - One-to-One, One-to-Many, Many-to-Many relationships
//!   - Cascade operations (ON DELETE CASCADE, SET NULL, RESTRICT)
//!   - Back-references with automatic join queries
//!   - Lazy and eager loading strategies
//!
//! - **Transaction Support**:
//!   - ACID-compliant transactions with savepoints
//!   - Nested transaction support via savepoints
//!   - Automatic rollback on error
//!   - Connection pooling for optimal performance
//!
//! - **Schema Management**:
//!   - Schema introspection and validation
//!   - Migration system with version tracking
//!   - Type-safe column operations
//!   - Foreign key constraint validation
//!
//! - **Performance Optimizations**:
//!   - Connection pooling with configurable limits
//!   - Prepared statement caching
//!   - Bulk insert/update operations
//!   - Parallel query execution
//!
//! # Usage Examples
//!
//! ## Basic Query Execution
//!
//! ```rust,ignore
//! use pg::{Connection, QueryBuilder, Operator, PoolConfig};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Connect to PostgreSQL
//! let conn = Connection::new("postgresql://user:pass@localhost/dbname", PoolConfig::default()).await?;
//!
//! // Build and execute a query
//! let query = QueryBuilder::new("users")?
//!     .select(vec!["id".to_string(), "name".to_string(), "email".to_string()])?
//!     .where_clause("age", Operator::Gt, "18".into())?
//!     .order_by("name", pg::OrderDirection::Asc)?
//!     .limit(10)?;
//!
//! let rows = query.fetch_rows(&conn).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Transaction with Savepoints
//!
//! ```rust,ignore
//! use pg::{Connection, Transaction, PoolConfig};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let conn = Connection::new("postgresql://localhost/db", PoolConfig::default()).await?;
//! let mut txn = Transaction::begin(&conn).await?;
//!
//! // Insert user
//! txn.execute("INSERT INTO users (name) VALUES ($1)").await?;
//!
//! // Create savepoint
//! txn.savepoint("sp1").await?;
//!
//! // This might fail
//! if let Err(_) = txn.execute("INSERT INTO users (name) VALUES ($1)").await {
//!     // Rollback to savepoint, keeping Alice's insert
//!     txn.rollback_to_savepoint("sp1").await?;
//! }
//!
//! // Commit the transaction
//! txn.commit().await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Complex Query with Joins and Window Functions
//!
//! ```rust,ignore
//! use pg::{QueryBuilder, JoinType, WindowFunction, WindowSpec, OrderDirection};
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let query = QueryBuilder::new("orders")?
//!     .select(vec!["orders.id".into(), "orders.amount".into(), "customers.name".into()])?
//!     .join(
//!         JoinType::Inner,
//!         "customers",
//!         "orders.customer_id",
//!         "customers.id"
//!     )?
//!     .window_function(
//!         WindowFunction::RowNumber,
//!         WindowSpec::new()
//!             .partition_by(vec!["customers.id".into()])?
//!             .order_by("orders.created_at", OrderDirection::Desc)?
//!     )?
//!     .order_by("orders.created_at", OrderDirection::Desc)?;
//!
//! // Execute query via sqlx...
//! # Ok(())
//! # }
//! ```
//!
//! # Async Runtime
//!
//! This crate requires an async runtime (Tokio) to function. All database operations
//! are async and must be awaited by the language binding layer.
//! The binding layer owns language-level scheduling. Core operations stay in
//! Rust and expose async primitives plus explicit blocking facade types.
//!
//! # Safety
//!
//! All SQL queries are parameterized to prevent SQL injection attacks. Table names,
//! column names, and operators are validated before query execution. Foreign key
//! references are validated to ensure referential integrity.
//!
//! # Thread Safety
//!
//! The `Connection` type is **not** `Send` or `Sync` by design. Each async task should
//! obtain its own connection from the pool. The connection pool itself is thread-safe
//! and can be cloned cheaply across threads.

// ---------------------------------------------------------------------
// Layered modules — see `.aw/tech-design/projects/pg/specs/pg-mod-boundary.md`
//
//   driver  = psycopg / asyncpg equivalent (raw async I/O + blocking facade)
//   orm     = SQLAlchemy equivalent      (models, query builder, validation)
//   migrate = Alembic equivalent         (versioning + history viz)
//
// Cross-cutting modules (`cli`, `metrics`) stay at the crate root.
// Backward compatibility: flat `pub use <layer>::*;` shims below preserve
// the historic `pg::<Foo>` paths for every existing caller.
// ---------------------------------------------------------------------

pub mod driver;
pub mod migrate;
pub mod orm;

/// CLI migration tool for database management.
pub mod cli;

/// Connection pool metrics and monitoring.
pub mod metrics;

// Flat compatibility surface — every prior crate-root export resolves
// here unchanged.
pub use driver::*;
pub use migrate::*;
pub use orm::*;

// Aggregator for the historic `pg::blocking::*` namespace.
// Combines the driver-side blocking facade with the orm-side Session
// surface. Pure `pub use` only — no orm symbol is depended on from
// inside `driver/`, so the one-way layer boundary (driver → orm
// forbidden) is preserved.
pub mod blocking {
    pub use crate::driver::blocking::*;
    pub use crate::orm::blocking::*;
}

// CLI re-exports
pub use cli::{CliResult, MigrationCli, MigrationCliConfig, MigrationCommand};

// Validation re-exports from cclab-shield (new API)
// These provide Pydantic-style validation with Rust performance
pub use ::schema::{
    coerce_value,
    custom_error,
    field_error,
    // Validation functions
    validate,
    validate_value,
    validate_with_context,
    BoxedComputedField,
    BoxedFieldValidator,
    BoxedModelValidator,
    // Coercion
    CoercionMode,
    // Computed fields
    ComputedField,
    ComputedFieldCollection,
    ErrorType,
    FieldDescriptor,
    // Validators (new shield API)
    FieldValidator,
    FnComputedField,
    FnFieldValidator,
    FnModelValidator,
    ListConstraints,
    ModelValidator,
    NumericConstraints,
    // Constraints
    StringConstraints,
    StringFormat,
    // Core types
    TypeDescriptor,
    // Error types (new shield types)
    ValidationResult,
    ValidatorCollection,
    ValidatorContext,
    ValidatorMode,
    Value,
};

// Re-export shield's ValidationError/ValidationErrors with different names
// to avoid confusion with compat layer
pub use ::schema::ValidationContext as ShieldValidationContext;
pub use ::schema::ValidationError as ShieldValidationError;
pub use ::schema::ValidationErrors as ShieldValidationErrors;

// Compatibility re-exports (backward compatible with old pydantic_validation API)
// These maintain the old API signatures for easier migration
pub use compat::{
    ComputedFieldConfig, EmailValidator, FieldValidatorConfig, LengthValidator,
    ModelValidatorConfig, PatternValidator, RangeValidator, UrlValidator, ValidationError,
    ValidationErrors, ValidationMode, ValidationRegistry,
};

// History visualization re-exports
pub use history_vis::{
    AsciiConfig, AsciiRenderer, ExportFormat, HistoryExporter, HistoryVisualizer, MigrationNode,
    MigrationTree,
};

// Bulk operations re-exports
pub use bulk::{BulkConfig, BulkExecutor, BulkResult};

// Pool metrics re-exports
pub use metrics::{HealthCheck, HealthStatus, LatencyStats, MetricsCollector, PoolMetrics};

// Back-reference re-exports
pub use backref::{BackRefConfig, BackRefLoader, EagerLoader, EagerRelation};

pub use mamba_core::{DataBridgeError, Result};
