//! PostgreSQL query builder.
//!
//! This module provides a type-safe query builder for constructing SQL queries,
//! similar to cclab-nebula's query builder but for SQL.
//!
//! # Examples
//!
//! ## SELECT Query
//!
//! ```ignore
//! use pg::{QueryBuilder, Operator, OrderDirection, ExtractedValue};
//!
//! let qb = QueryBuilder::new("users")?
//!     .select(vec!["id".to_string(), "name".to_string()])?
//!     .where_clause("age", Operator::Gte, ExtractedValue::Int(18))?
//!     .where_clause("active", Operator::Eq, ExtractedValue::Bool(true))?
//!     .order_by("name", OrderDirection::Asc)?
//!     .limit(10)
//!     .offset(20);
//!
//! let (sql, params) = qb.build();
//! // Result: "SELECT id, name FROM users WHERE age >= $1 AND active = $2 ORDER BY name ASC LIMIT $3 OFFSET $4"
//! ```
//!
//! ## INSERT Query
//!
//! ```ignore
//! use pg::{QueryBuilder, ExtractedValue};
//!
//! let qb = QueryBuilder::new("users")?;
//! let values = vec![
//!     ("name".to_string(), ExtractedValue::String("Alice".to_string())),
//!     ("age".to_string(), ExtractedValue::Int(30)),
//! ];
//! let (sql, params) = qb.build_insert(&values)?;
//! // Result: "INSERT INTO users (name, age) VALUES ($1, $2) RETURNING *"
//! ```
//!
//! ## UPDATE Query
//!
//! ```ignore
//! use pg::{QueryBuilder, Operator, ExtractedValue};
//!
//! let qb = QueryBuilder::new("users")?
//!     .where_clause("id", Operator::Eq, ExtractedValue::Int(42))?;
//! let values = vec![
//!     ("name".to_string(), ExtractedValue::String("Bob".to_string())),
//!     ("age".to_string(), ExtractedValue::Int(35)),
//! ];
//! let (sql, params) = qb.build_update(&values)?;
//! // Result: "UPDATE users SET name = $1, age = $2 WHERE id = $3"
//! ```
//!
//! ## DELETE Query
//!
//! ```ignore
//! use pg::{QueryBuilder, Operator, ExtractedValue};
//!
//! let qb = QueryBuilder::new("users")?
//!     .where_clause("id", Operator::Eq, ExtractedValue::Int(42))?;
//! let (sql, params) = qb.build_delete();
//! // Result: "DELETE FROM users WHERE id = $1"
//! ```

mod builder;
mod execute;
mod helpers;
mod join;
mod modify;
mod select;
mod types;
mod window;

#[cfg(test)]
mod tests;

// Re-export all public types
pub use builder::QueryBuilder;
pub use join::{JoinClause, JoinCondition};
pub use types::{
    AggregateFunction, CommonTableExpression, HavingCondition, JoinType, Operator, OrderDirection,
    SetOperation, SetQuery, Subquery,
};
pub use window::{WindowExpression, WindowFunction, WindowSpec};
