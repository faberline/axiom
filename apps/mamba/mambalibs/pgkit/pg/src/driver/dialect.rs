//! Database dialect abstraction for multi-database support
//!
//! This module provides traits and implementations for database-specific
//! SQL generation, enabling cclab-titan to support PostgreSQL, SQLite, and MySQL.
//!
//! # Architecture
//!
//! The dialect system consists of two main traits:
//! - `Dialect`: Handles SQL syntax differences (quoting, placeholders, etc.)
//! - `DatabasePool`: Abstracts connection pooling across different sqlx drivers
//!
//! # Example
//!
//! ```ignore
//! use pg::dialect::{Dialect, PostgresDialect, SqliteDialect};
//!
//! let pg = PostgresDialect;
//! assert_eq!(pg.quote_identifier("table"), "\"table\"");
//! assert_eq!(pg.param_placeholder(1), "$1");
//!
//! let sqlite = SqliteDialect;
//! assert_eq!(sqlite.quote_identifier("table"), "\"table\"");
//! assert_eq!(sqlite.param_placeholder(1), "?");
//! ```

use std::fmt;

// ============================================================================
// Dialect Trait
// ============================================================================

/// SQL dialect abstraction for database-specific syntax
pub trait Dialect: Send + Sync + fmt::Debug {
    /// Get the dialect name (e.g., "postgresql", "sqlite", "mysql")
    fn name(&self) -> &'static str;

    /// Quote an identifier (table name, column name, etc.)
    fn quote_identifier(&self, ident: &str) -> String;

    /// Get parameter placeholder for the given index (1-based)
    fn param_placeholder(&self, index: usize) -> String;

    /// Check if the dialect supports RETURNING clause
    fn supports_returning(&self) -> bool;

    /// Get LIMIT/OFFSET syntax
    fn limit_offset(&self, limit: Option<u64>, offset: Option<u64>) -> String {
        match (limit, offset) {
            (Some(l), Some(o)) => format!("LIMIT {} OFFSET {}", l, o),
            (Some(l), None) => format!("LIMIT {}", l),
            (None, Some(o)) => format!("OFFSET {}", o),
            (None, None) => String::new(),
        }
    }

    /// Get the syntax for boolean literal
    fn boolean_literal(&self, value: bool) -> &'static str {
        if value {
            "TRUE"
        } else {
            "FALSE"
        }
    }

    /// Get current timestamp expression
    fn current_timestamp(&self) -> &'static str;

    /// Get the syntax for inserting with RETURNING (if supported)
    fn insert_returning(
        &self,
        table: &str,
        columns: &[&str],
        returning: &[&str],
    ) -> Option<String> {
        if !self.supports_returning() {
            return None;
        }

        let cols = columns.join(", ");
        let placeholders: Vec<String> = (1..=columns.len())
            .map(|i| self.param_placeholder(i))
            .collect();
        let values = placeholders.join(", ");
        let ret = returning.join(", ");

        Some(format!(
            "INSERT INTO {} ({}) VALUES ({}) RETURNING {}",
            self.quote_identifier(table),
            cols,
            values,
            ret
        ))
    }

    /// Check if the dialect supports ON CONFLICT (upsert)
    fn supports_on_conflict(&self) -> bool;

    /// Get the upsert (ON CONFLICT) syntax
    fn upsert_syntax(&self, conflict_columns: &[&str], update_columns: &[&str]) -> String;
}

// ============================================================================
// PostgreSQL Dialect
// ============================================================================

/// PostgreSQL dialect implementation
#[derive(Debug, Clone, Copy, Default)]
pub struct PostgresDialect;

impl Dialect for PostgresDialect {
    fn name(&self) -> &'static str {
        "postgresql"
    }

    fn quote_identifier(&self, ident: &str) -> String {
        format!("\"{}\"", ident.replace('"', "\"\""))
    }

    fn param_placeholder(&self, index: usize) -> String {
        format!("${}", index)
    }

    fn supports_returning(&self) -> bool {
        true
    }

    fn current_timestamp(&self) -> &'static str {
        "NOW()"
    }

    fn supports_on_conflict(&self) -> bool {
        true
    }

    fn upsert_syntax(&self, conflict_columns: &[&str], update_columns: &[&str]) -> String {
        let conflict = conflict_columns.join(", ");
        let updates: Vec<String> = update_columns
            .iter()
            .map(|c| format!("{} = EXCLUDED.{}", c, c))
            .collect();
        format!(
            "ON CONFLICT ({}) DO UPDATE SET {}",
            conflict,
            updates.join(", ")
        )
    }
}

// ============================================================================
// SQLite Dialect
// ============================================================================

/// SQLite dialect implementation
#[derive(Debug, Clone, Copy, Default)]
pub struct SqliteDialect;

impl Dialect for SqliteDialect {
    fn name(&self) -> &'static str {
        "sqlite"
    }

    fn quote_identifier(&self, ident: &str) -> String {
        format!("\"{}\"", ident.replace('"', "\"\""))
    }

    fn param_placeholder(&self, _index: usize) -> String {
        "?".to_string()
    }

    fn supports_returning(&self) -> bool {
        true // SQLite 3.35+ supports RETURNING
    }

    fn current_timestamp(&self) -> &'static str {
        "CURRENT_TIMESTAMP"
    }

    fn supports_on_conflict(&self) -> bool {
        true
    }

    fn upsert_syntax(&self, conflict_columns: &[&str], update_columns: &[&str]) -> String {
        let conflict = conflict_columns.join(", ");
        let updates: Vec<String> = update_columns
            .iter()
            .map(|c| format!("{} = excluded.{}", c, c))
            .collect();
        format!(
            "ON CONFLICT({}) DO UPDATE SET {}",
            conflict,
            updates.join(", ")
        )
    }
}

// ============================================================================
// MySQL Dialect
// ============================================================================

/// MySQL dialect implementation
#[derive(Debug, Clone, Copy, Default)]
pub struct MysqlDialect;

impl Dialect for MysqlDialect {
    fn name(&self) -> &'static str {
        "mysql"
    }

    fn quote_identifier(&self, ident: &str) -> String {
        format!("`{}`", ident.replace('`', "``"))
    }

    fn param_placeholder(&self, _index: usize) -> String {
        "?".to_string()
    }

    fn supports_returning(&self) -> bool {
        false // MySQL doesn't support RETURNING
    }

    fn current_timestamp(&self) -> &'static str {
        "NOW()"
    }

    fn boolean_literal(&self, value: bool) -> &'static str {
        if value {
            "1"
        } else {
            "0"
        }
    }

    fn supports_on_conflict(&self) -> bool {
        true // MySQL uses ON DUPLICATE KEY UPDATE
    }

    fn upsert_syntax(&self, _conflict_columns: &[&str], update_columns: &[&str]) -> String {
        // MySQL uses ON DUPLICATE KEY UPDATE (ignores conflict columns)
        let updates: Vec<String> = update_columns
            .iter()
            .map(|c| format!("{} = VALUES({})", c, c))
            .collect();
        format!("ON DUPLICATE KEY UPDATE {}", updates.join(", "))
    }
}

// ============================================================================
// Dialect Detection
// ============================================================================

/// Detect dialect from a database connection URI
pub fn detect_dialect(uri: &str) -> Box<dyn Dialect> {
    if uri.starts_with("postgres://") || uri.starts_with("postgresql://") {
        Box::new(PostgresDialect)
    } else if uri.starts_with("sqlite://") || uri.starts_with("sqlite:") {
        Box::new(SqliteDialect)
    } else if uri.starts_with("mysql://") || uri.starts_with("mariadb://") {
        Box::new(MysqlDialect)
    } else {
        // Default to PostgreSQL
        Box::new(PostgresDialect)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_postgres_dialect() {
        let dialect = PostgresDialect;

        assert_eq!(dialect.name(), "postgresql");
        assert_eq!(dialect.quote_identifier("users"), "\"users\"");
        assert_eq!(dialect.quote_identifier("my\"table"), "\"my\"\"table\"");
        assert_eq!(dialect.param_placeholder(1), "$1");
        assert_eq!(dialect.param_placeholder(5), "$5");
        assert!(dialect.supports_returning());
        assert_eq!(dialect.current_timestamp(), "NOW()");
        assert_eq!(dialect.boolean_literal(true), "TRUE");
    }

    #[test]
    fn test_sqlite_dialect() {
        let dialect = SqliteDialect;

        assert_eq!(dialect.name(), "sqlite");
        assert_eq!(dialect.quote_identifier("users"), "\"users\"");
        assert_eq!(dialect.param_placeholder(1), "?");
        assert_eq!(dialect.param_placeholder(5), "?");
        assert!(dialect.supports_returning());
        assert_eq!(dialect.current_timestamp(), "CURRENT_TIMESTAMP");
    }

    #[test]
    fn test_mysql_dialect() {
        let dialect = MysqlDialect;

        assert_eq!(dialect.name(), "mysql");
        assert_eq!(dialect.quote_identifier("users"), "`users`");
        assert_eq!(dialect.quote_identifier("my`table"), "`my``table`");
        assert_eq!(dialect.param_placeholder(1), "?");
        assert!(!dialect.supports_returning());
        assert_eq!(dialect.current_timestamp(), "NOW()");
        assert_eq!(dialect.boolean_literal(true), "1");
        assert_eq!(dialect.boolean_literal(false), "0");
    }

    #[test]
    fn test_limit_offset() {
        let dialect = PostgresDialect;

        assert_eq!(dialect.limit_offset(Some(10), None), "LIMIT 10");
        assert_eq!(dialect.limit_offset(None, Some(5)), "OFFSET 5");
        assert_eq!(dialect.limit_offset(Some(10), Some(5)), "LIMIT 10 OFFSET 5");
        assert_eq!(dialect.limit_offset(None, None), "");
    }

    #[test]
    fn test_upsert_syntax() {
        let pg = PostgresDialect;
        let syntax = pg.upsert_syntax(&["id"], &["name", "email"]);
        assert!(syntax.contains("ON CONFLICT (id)"));
        assert!(syntax.contains("EXCLUDED.name"));

        let mysql = MysqlDialect;
        let syntax = mysql.upsert_syntax(&["id"], &["name", "email"]);
        assert!(syntax.contains("ON DUPLICATE KEY UPDATE"));
        assert!(syntax.contains("VALUES(name)"));
    }

    #[test]
    fn test_detect_dialect() {
        let pg = detect_dialect("postgres://localhost/db");
        assert_eq!(pg.name(), "postgresql");

        let sqlite = detect_dialect("sqlite:memory:");
        assert_eq!(sqlite.name(), "sqlite");

        let mysql = detect_dialect("mysql://localhost/db");
        assert_eq!(mysql.name(), "mysql");
    }
}
