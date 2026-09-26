//! Back-reference loading for relationships.
//!
//! This module provides utilities for loading related records through back-references.
//! When a table has foreign keys pointing to another table, this module allows
//! querying from the parent side (e.g., User -> Posts).
//!
//! # Example
//!
//! ```rust,ignore
//! use pg::{Connection, BackRefLoader, BackRefConfig, PoolConfig};
//!
//! let conn = Connection::new(&uri, PoolConfig::default()).await?;
//!
//! // Load all posts by a user
//! let loader = BackRefLoader::new(&conn);
//! let posts = loader.load_related("users", user_id, "posts", "author_id").await?;
//!
//! // With configuration (ordering, limit)
//! let config = BackRefConfig::new()
//!     .order_by("created_at", false)
//!     .limit(10);
//! let recent_posts = loader.load_related_with_config(
//!     "users", user_id, "posts", "author_id", &config
//! ).await?;
//! ```

use crate::{
    row_to_extracted, BackRef, Connection, DataBridgeError, ExtractedValue, QueryBuilder, Result,
    Row,
};
use sqlx::postgres::PgRow;
use sqlx::Row as SqlxRow;
use std::collections::HashMap;
use tracing::instrument;

/// Configuration for back-reference queries.
#[derive(Debug, Clone)]
pub struct BackRefConfig {
    /// Columns to select (None = all columns).
    pub select_columns: Option<Vec<String>>,
    /// Column to order by.
    pub order_by: Option<String>,
    /// Order direction (true = ASC, false = DESC).
    pub order_asc: bool,
    /// Maximum number of records to return.
    pub limit: Option<i64>,
    /// Number of records to skip.
    pub offset: Option<i64>,
    /// Additional WHERE conditions (column, operator, value).
    pub filters: Vec<(String, String, ExtractedValue)>,
}

impl Default for BackRefConfig {
    fn default() -> Self {
        Self {
            select_columns: None,
            order_by: None,
            order_asc: true, // Default to ascending order
            limit: None,
            offset: None,
            filters: Vec::new(),
        }
    }
}

impl BackRefConfig {
    /// Create a new empty configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set columns to select.
    pub fn select(mut self, columns: Vec<String>) -> Self {
        self.select_columns = Some(columns);
        self
    }

    /// Set order by column and direction.
    pub fn order_by(mut self, column: &str, ascending: bool) -> Self {
        self.order_by = Some(column.to_string());
        self.order_asc = ascending;
        self
    }

    /// Set maximum records to return.
    pub fn limit(mut self, limit: i64) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Set number of records to skip.
    pub fn offset(mut self, offset: i64) -> Self {
        self.offset = Some(offset);
        self
    }

    /// Add a filter condition.
    pub fn filter(mut self, column: &str, operator: &str, value: ExtractedValue) -> Self {
        self.filters
            .push((column.to_string(), operator.to_string(), value));
        self
    }
}

/// Loader for back-reference queries.
///
/// Enables loading related records from the "many" side of a relationship
/// when you have a record from the "one" side.
#[derive(Debug)]
pub struct BackRefLoader<'a> {
    conn: &'a Connection,
}

impl<'a> BackRefLoader<'a> {
    /// Create a new loader with a connection reference.
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Load related records through a back-reference.
    ///
    /// # Arguments
    ///
    /// * `parent_table` - The parent table name (e.g., "users")
    /// * `parent_id` - The parent record ID
    /// * `child_table` - The child table name (e.g., "posts")
    /// * `foreign_key` - The FK column in child table (e.g., "author_id")
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Get all posts by user with id=1
    /// let posts = loader.load_related("users", 1, "posts", "author_id").await?;
    /// ```
    #[instrument(skip(self), fields(parent_table, child_table, foreign_key))]
    pub async fn load_related(
        &self,
        parent_table: &str,
        parent_id: i64,
        child_table: &str,
        foreign_key: &str,
    ) -> Result<Vec<Row>> {
        self.load_related_with_config(
            parent_table,
            parent_id,
            child_table,
            foreign_key,
            &BackRefConfig::default(),
        )
        .await
    }

    /// Load related records with configuration options.
    #[instrument(skip(self, config), fields(parent_table, child_table, foreign_key))]
    pub async fn load_related_with_config(
        &self,
        parent_table: &str,
        parent_id: i64,
        child_table: &str,
        foreign_key: &str,
        config: &BackRefConfig,
    ) -> Result<Vec<Row>> {
        // Validate identifiers
        QueryBuilder::validate_identifier(parent_table)?;
        QueryBuilder::validate_identifier(child_table)?;
        QueryBuilder::validate_identifier(foreign_key)?;

        // Build SELECT clause
        let select_clause = match &config.select_columns {
            Some(cols) => {
                let mut quoted = Vec::new();
                for col in cols {
                    QueryBuilder::validate_identifier(col)?;
                    quoted.push(QueryBuilder::quote_identifier(col));
                }
                quoted.join(", ")
            }
            None => "*".to_string(),
        };

        // Build base query
        let mut sql = format!(
            "SELECT {} FROM {} WHERE {} = $1",
            select_clause,
            QueryBuilder::quote_identifier(child_table),
            QueryBuilder::quote_identifier(foreign_key)
        );

        // Add filters
        let mut param_num = 2;
        for (col, op, _) in &config.filters {
            QueryBuilder::validate_identifier(col)?;
            sql.push_str(&format!(
                " AND {} {} ${}",
                QueryBuilder::quote_identifier(col),
                op,
                param_num
            ));
            param_num += 1;
        }

        // Add ORDER BY
        if let Some(order_col) = &config.order_by {
            QueryBuilder::validate_identifier(order_col)?;
            let direction = if config.order_asc { "ASC" } else { "DESC" };
            sql.push_str(&format!(
                " ORDER BY {} {}",
                QueryBuilder::quote_identifier(order_col),
                direction
            ));
        }

        // Add LIMIT/OFFSET
        if let Some(limit) = config.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }
        if let Some(offset) = config.offset {
            sql.push_str(&format!(" OFFSET {}", offset));
        }

        // Execute query
        let mut query = sqlx::query(&sql).bind(parent_id);

        for (_, _, value) in &config.filters {
            query = Self::bind_value(query, value);
        }

        let rows: Vec<PgRow> = query.fetch_all(self.conn.pool()).await?;

        // Convert to Row structs
        let mut results = Vec::with_capacity(rows.len());
        for pg_row in rows {
            let extracted = row_to_extracted(&pg_row)?;
            results.push(Row::new(extracted));
        }

        Ok(results)
    }

    /// Load related records for multiple parent IDs (batch loading).
    ///
    /// Returns a map from parent ID to its related records.
    /// This is more efficient than calling `load_related` multiple times.
    #[instrument(skip(self, parent_ids), fields(child_table, foreign_key, count = parent_ids.len()))]
    pub async fn load_related_batch(
        &self,
        parent_table: &str,
        parent_ids: &[i64],
        child_table: &str,
        foreign_key: &str,
    ) -> Result<HashMap<i64, Vec<Row>>> {
        self.load_related_batch_with_config(
            parent_table,
            parent_ids,
            child_table,
            foreign_key,
            &BackRefConfig::default(),
        )
        .await
    }

    /// Load related records for multiple parent IDs with configuration.
    #[instrument(skip(self, parent_ids, config), fields(child_table, foreign_key, count = parent_ids.len()))]
    pub async fn load_related_batch_with_config(
        &self,
        parent_table: &str,
        parent_ids: &[i64],
        child_table: &str,
        foreign_key: &str,
        config: &BackRefConfig,
    ) -> Result<HashMap<i64, Vec<Row>>> {
        if parent_ids.is_empty() {
            return Ok(HashMap::new());
        }

        // Validate identifiers
        QueryBuilder::validate_identifier(parent_table)?;
        QueryBuilder::validate_identifier(child_table)?;
        QueryBuilder::validate_identifier(foreign_key)?;

        // Build SELECT clause - always include FK column for grouping
        let select_clause = match &config.select_columns {
            Some(cols) => {
                let mut quoted = Vec::new();
                // Ensure FK is included for grouping
                let fk_quoted = QueryBuilder::quote_identifier(foreign_key);
                if !cols.contains(&foreign_key.to_string()) {
                    quoted.push(fk_quoted);
                }
                for col in cols {
                    QueryBuilder::validate_identifier(col)?;
                    quoted.push(QueryBuilder::quote_identifier(col));
                }
                quoted.join(", ")
            }
            None => "*".to_string(),
        };

        // Build query with ANY for batch
        let mut sql = format!(
            "SELECT {} FROM {} WHERE {} = ANY($1)",
            select_clause,
            QueryBuilder::quote_identifier(child_table),
            QueryBuilder::quote_identifier(foreign_key)
        );

        // Add ORDER BY
        if let Some(order_col) = &config.order_by {
            QueryBuilder::validate_identifier(order_col)?;
            let direction = if config.order_asc { "ASC" } else { "DESC" };
            sql.push_str(&format!(
                " ORDER BY {} {}",
                QueryBuilder::quote_identifier(order_col),
                direction
            ));
        }

        // Execute query
        let rows: Vec<PgRow> = sqlx::query(&sql)
            .bind(parent_ids)
            .fetch_all(self.conn.pool())
            .await?;

        // Group results by parent ID
        let mut results: HashMap<i64, Vec<Row>> = HashMap::new();

        // Initialize all parent IDs with empty vectors
        for &id in parent_ids {
            results.insert(id, Vec::new());
        }

        for pg_row in rows {
            let extracted = row_to_extracted(&pg_row)?;

            // Get the foreign key value to determine parent
            let fk_value = pg_row.try_get::<i64, _>(foreign_key).map_err(|e| {
                DataBridgeError::Query(format!("Failed to get FK column {}: {}", foreign_key, e))
            })?;

            let row = Row::new(extracted);
            results.entry(fk_value).or_default().push(row);
        }

        Ok(results)
    }

    /// Count related records for a parent.
    #[instrument(skip(self), fields(parent_table, child_table, foreign_key))]
    pub async fn count_related(
        &self,
        parent_table: &str,
        parent_id: i64,
        child_table: &str,
        foreign_key: &str,
    ) -> Result<i64> {
        QueryBuilder::validate_identifier(parent_table)?;
        QueryBuilder::validate_identifier(child_table)?;
        QueryBuilder::validate_identifier(foreign_key)?;

        let sql = format!(
            "SELECT COUNT(*) as count FROM {} WHERE {} = $1",
            QueryBuilder::quote_identifier(child_table),
            QueryBuilder::quote_identifier(foreign_key)
        );

        let row: PgRow = sqlx::query(&sql)
            .bind(parent_id)
            .fetch_one(self.conn.pool())
            .await?;

        let count: i64 = row.try_get("count")?;
        Ok(count)
    }

    /// Check if a parent has any related records.
    #[instrument(skip(self), fields(parent_table, child_table, foreign_key))]
    pub async fn has_related(
        &self,
        parent_table: &str,
        parent_id: i64,
        child_table: &str,
        foreign_key: &str,
    ) -> Result<bool> {
        QueryBuilder::validate_identifier(parent_table)?;
        QueryBuilder::validate_identifier(child_table)?;
        QueryBuilder::validate_identifier(foreign_key)?;

        let sql = format!(
            "SELECT EXISTS(SELECT 1 FROM {} WHERE {} = $1) as has_related",
            QueryBuilder::quote_identifier(child_table),
            QueryBuilder::quote_identifier(foreign_key)
        );

        let row: PgRow = sqlx::query(&sql)
            .bind(parent_id)
            .fetch_one(self.conn.pool())
            .await?;

        let has: bool = row.try_get("has_related")?;
        Ok(has)
    }

    /// Load related records using a BackRef metadata struct.
    ///
    /// This is a convenience method when you have the BackRef from
    /// SchemaInspector::get_backreferences().
    #[instrument(skip(self, backref), fields(source = %backref.source_table, target = %backref.target_table))]
    pub async fn load_from_backref(&self, backref: &BackRef, parent_id: i64) -> Result<Vec<Row>> {
        self.load_related(
            &backref.target_table,
            parent_id,
            &backref.source_table,
            &backref.source_column,
        )
        .await
    }

    /// Load related records using a BackRef with configuration.
    #[instrument(skip(self, backref, config), fields(source = %backref.source_table, target = %backref.target_table))]
    pub async fn load_from_backref_with_config(
        &self,
        backref: &BackRef,
        parent_id: i64,
        config: &BackRefConfig,
    ) -> Result<Vec<Row>> {
        self.load_related_with_config(
            &backref.target_table,
            parent_id,
            &backref.source_table,
            &backref.source_column,
            config,
        )
        .await
    }

    /// Helper to bind ExtractedValue to a query.
    fn bind_value<'q>(
        query: sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments>,
        value: &'q ExtractedValue,
    ) -> sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments> {
        match value {
            ExtractedValue::Null => query.bind(None::<i64>),
            ExtractedValue::Bool(v) => query.bind(*v),
            ExtractedValue::SmallInt(v) => query.bind(*v),
            ExtractedValue::Int(v) => query.bind(*v),
            ExtractedValue::BigInt(v) => query.bind(*v),
            ExtractedValue::Float(v) => query.bind(*v),
            ExtractedValue::Double(v) => query.bind(*v),
            ExtractedValue::String(v) => query.bind(v.as_str()),
            ExtractedValue::Uuid(v) => query.bind(*v),
            ExtractedValue::Timestamp(v) => query.bind(*v),
            ExtractedValue::TimestampTz(v) => query.bind(*v),
            ExtractedValue::Date(v) => query.bind(*v),
            ExtractedValue::Time(v) => query.bind(*v),
            ExtractedValue::Decimal(v) => query.bind(*v),
            ExtractedValue::Json(v) => query.bind(v.clone()),
            ExtractedValue::Bytes(v) => query.bind(v.as_slice()),
            ExtractedValue::Array(_) => {
                // Arrays require special handling - for now, return unmodified
                // This should rarely be used in filter conditions
                query
            }
        }
    }
}

/// Eager loader for batch-loading related records.
///
/// Useful when loading a list of parent records and wanting to
/// include their related children in one efficient query.
#[derive(Debug)]
pub struct EagerLoader<'a> {
    conn: &'a Connection,
    relations: Vec<EagerRelation>,
}

/// Configuration for an eager-loaded relation.
#[derive(Debug, Clone)]
pub struct EagerRelation {
    /// Name/key for this relation in results.
    pub name: String,
    /// Child table name.
    pub child_table: String,
    /// Foreign key column in child table.
    pub foreign_key: String,
    /// Configuration for the query.
    pub config: BackRefConfig,
}

impl EagerRelation {
    /// Create a new eager relation.
    pub fn new(name: &str, child_table: &str, foreign_key: &str) -> Self {
        Self {
            name: name.to_string(),
            child_table: child_table.to_string(),
            foreign_key: foreign_key.to_string(),
            config: BackRefConfig::default(),
        }
    }

    /// Set configuration for this relation.
    pub fn with_config(mut self, config: BackRefConfig) -> Self {
        self.config = config;
        self
    }
}

impl<'a> EagerLoader<'a> {
    /// Create a new eager loader.
    pub fn new(conn: &'a Connection) -> Self {
        Self {
            conn,
            relations: Vec::new(),
        }
    }

    /// Add a relation to eager load.
    pub fn include(mut self, relation: EagerRelation) -> Self {
        self.relations.push(relation);
        self
    }

    /// Load all configured relations for the given parent records.
    ///
    /// Returns a map from relation name to (parent_id -> rows).
    #[instrument(skip(self, parent_table, parent_ids), fields(
        parent_table,
        parent_count = parent_ids.len(),
        relation_count = self.relations.len()
    ))]
    pub async fn load(
        &self,
        parent_table: &str,
        parent_ids: &[i64],
    ) -> Result<HashMap<String, HashMap<i64, Vec<Row>>>> {
        let loader = BackRefLoader::new(self.conn);
        let mut results = HashMap::new();

        for relation in &self.relations {
            let relation_data = loader
                .load_related_batch_with_config(
                    parent_table,
                    parent_ids,
                    &relation.child_table,
                    &relation.foreign_key,
                    &relation.config,
                )
                .await?;

            results.insert(relation.name.clone(), relation_data);
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backref_config_builder() {
        let config = BackRefConfig::new()
            .select(vec!["id".to_string(), "title".to_string()])
            .order_by("created_at", false)
            .limit(10)
            .offset(5);

        assert_eq!(
            config.select_columns,
            Some(vec!["id".to_string(), "title".to_string()])
        );
        assert_eq!(config.order_by, Some("created_at".to_string()));
        assert!(!config.order_asc);
        assert_eq!(config.limit, Some(10));
        assert_eq!(config.offset, Some(5));
    }

    #[test]
    fn test_backref_config_with_filter() {
        let config = BackRefConfig::new()
            .filter(
                "status",
                "=",
                ExtractedValue::String("published".to_string()),
            )
            .filter("views", ">", ExtractedValue::Int(100));

        assert_eq!(config.filters.len(), 2);
        assert_eq!(config.filters[0].0, "status");
        assert_eq!(config.filters[0].1, "=");
        assert_eq!(config.filters[1].0, "views");
    }

    #[test]
    fn test_eager_relation_builder() {
        let relation = EagerRelation::new("posts", "posts", "author_id")
            .with_config(BackRefConfig::new().limit(5));

        assert_eq!(relation.name, "posts");
        assert_eq!(relation.child_table, "posts");
        assert_eq!(relation.foreign_key, "author_id");
        assert_eq!(relation.config.limit, Some(5));
    }

    #[test]
    fn test_backref_config_default() {
        let config = BackRefConfig::default();

        assert!(config.select_columns.is_none());
        assert!(config.order_by.is_none());
        assert!(config.order_asc);
        assert!(config.limit.is_none());
        assert!(config.offset.is_none());
        assert!(config.filters.is_empty());
    }
}
