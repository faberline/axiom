//! Async bulk operations with bounded concurrency.
//!
//! This module provides high-performance bulk insert, update, and delete
//! operations across concurrent batches.
//!
//! # Example
//!
//! ```rust,ignore
//! use pg::{Connection, BulkConfig, BulkExecutor};
//!
//! let conn = Connection::new(&uri, PoolConfig::default()).await?;
//! let config = BulkConfig::default().batch_size(1000);
//!
//! // Bulk insert 10,000 rows in parallel batches
//! let results = BulkExecutor::new(&conn, config)
//!     .insert_parallel("users", &rows)
//!     .await?;
//! ```

use crate::{Connection, DataBridgeError, ExtractedValue, QueryBuilder, Result};
use futures::stream::{self, StreamExt};
use sqlx::postgres::PgArguments;
use sqlx::Arguments;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{info, instrument};

/// Configuration for bulk operations.
#[derive(Debug, Clone)]
pub struct BulkConfig {
    /// Number of rows per batch (default: 1000)
    pub batch_size: usize,
    /// Maximum number of parallel batches (default: num_cpus)
    pub max_parallelism: usize,
    /// Whether to continue on error (default: false)
    pub continue_on_error: bool,
}

impl Default for BulkConfig {
    fn default() -> Self {
        Self {
            batch_size: 1000,
            max_parallelism: num_cpus::get(),
            continue_on_error: false,
        }
    }
}

impl BulkConfig {
    /// Create a new BulkConfig with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the batch size.
    pub fn batch_size(mut self, size: usize) -> Self {
        self.batch_size = size.max(1);
        self
    }

    /// Set the maximum parallelism.
    pub fn max_parallelism(mut self, max: usize) -> Self {
        self.max_parallelism = max.max(1);
        self
    }

    /// Set whether to continue on error.
    pub fn continue_on_error(mut self, continue_on_error: bool) -> Self {
        self.continue_on_error = continue_on_error;
        self
    }
}

/// Result of a bulk operation.
#[derive(Debug, Clone)]
pub struct BulkResult {
    /// Total rows processed successfully
    pub success_count: usize,
    /// Total rows that failed (if continue_on_error is true)
    pub error_count: usize,
    /// Errors encountered (if continue_on_error is true)
    pub errors: Vec<String>,
}

impl BulkResult {
    fn new() -> Self {
        Self {
            success_count: 0,
            error_count: 0,
            errors: Vec::new(),
        }
    }

    fn merge(&mut self, other: BulkResult) {
        self.success_count += other.success_count;
        self.error_count += other.error_count;
        self.errors.extend(other.errors);
    }
}

/// Executor for parallel bulk operations.
pub struct BulkExecutor {
    conn: Arc<Connection>,
    config: BulkConfig,
}

impl BulkExecutor {
    /// Create a new BulkExecutor with the given connection and config.
    pub fn new(conn: &Connection, config: BulkConfig) -> Self {
        Self {
            conn: Arc::new(conn.clone()),
            config,
        }
    }

    /// Perform parallel bulk insert.
    ///
    /// Splits the rows into batches and inserts them concurrently.
    /// Each batch is inserted using a single INSERT statement with RETURNING.
    #[instrument(skip(self, rows), fields(table = %table, total_rows = rows.len()))]
    pub async fn insert_parallel(
        &self,
        table: &str,
        rows: &[HashMap<String, ExtractedValue>],
    ) -> Result<BulkResult> {
        if rows.is_empty() {
            return Ok(BulkResult::new());
        }

        info!("Starting parallel bulk insert");
        QueryBuilder::validate_identifier(table)?;

        let batches: Vec<_> = rows
            .chunks(self.config.batch_size)
            .map(<[HashMap<String, ExtractedValue>]>::to_vec)
            .collect();
        let batch_count = batches.len();
        info!(
            batch_count,
            batch_size = self.config.batch_size,
            "Split into batches"
        );

        let conn = self.conn.clone();
        let table = table.to_string();
        let continue_on_error = self.config.continue_on_error;

        let max_parallelism = self.config.max_parallelism;
        let results: Vec<Result<BulkResult>> = stream::iter(batches)
            .map(|batch| {
                let conn = conn.clone();
                let table = table.clone();
                async move { Self::insert_batch(&conn, &table, &batch).await }
            })
            .buffer_unordered(max_parallelism)
            .collect()
            .await;

        // Aggregate results
        let mut final_result = BulkResult::new();
        for result in results {
            match result {
                Ok(r) => final_result.merge(r),
                Err(e) => {
                    if continue_on_error {
                        final_result.error_count += 1;
                        final_result.errors.push(e.to_string());
                    } else {
                        return Err(e);
                    }
                }
            }
        }

        info!(
            success = final_result.success_count,
            errors = final_result.error_count,
            "Parallel bulk insert complete"
        );
        Ok(final_result)
    }

    /// Insert a single batch of rows.
    async fn insert_batch(
        conn: &Connection,
        table: &str,
        rows: &[HashMap<String, ExtractedValue>],
    ) -> Result<BulkResult> {
        if rows.is_empty() {
            return Ok(BulkResult::new());
        }

        let first_row = &rows[0];
        let mut column_names: Vec<&String> = first_row.keys().collect();
        column_names.sort();

        for col in &column_names {
            QueryBuilder::validate_identifier(col)?;
        }

        let mut col_list = Vec::new();
        for s in &column_names {
            col_list.push(QueryBuilder::quote_identifier(s));
        }

        let mut sql = format!(
            "INSERT INTO {} ({}) VALUES ",
            QueryBuilder::quote_identifier(table),
            col_list.join(", ")
        );

        let mut param_num = 1;
        let mut values_clauses = Vec::with_capacity(rows.len());

        for _ in 0..rows.len() {
            let mut placeholders = Vec::new();
            for _ in 0..column_names.len() {
                placeholders.push(format!("${}", param_num));
                param_num += 1;
            }
            values_clauses.push(format!("({})", placeholders.join(", ")));
        }

        sql.push_str(&values_clauses.join(", "));

        let mut args = PgArguments::default();
        for row in rows {
            for col_name in &column_names {
                let value = row.get(*col_name).ok_or_else(|| {
                    DataBridgeError::Query("Required column not found in row data".to_string())
                })?;
                value.bind_to_arguments(&mut args)?;
            }
        }

        let result = sqlx::query_with(&sql, args)
            .execute(conn.pool())
            .await
            .map_err(|e| DataBridgeError::Query(format!("Batch insert failed: {}", e)))?;

        Ok(BulkResult {
            success_count: result.rows_affected() as usize,
            error_count: 0,
            errors: Vec::new(),
        })
    }

    /// Perform parallel bulk update.
    ///
    /// Updates rows in parallel batches. Each row must have an "id" field.
    #[instrument(skip(self, rows), fields(table = %table, total_rows = rows.len()))]
    pub async fn update_parallel(
        &self,
        table: &str,
        rows: &[HashMap<String, ExtractedValue>],
    ) -> Result<BulkResult> {
        if rows.is_empty() {
            return Ok(BulkResult::new());
        }

        info!("Starting parallel bulk update");
        QueryBuilder::validate_identifier(table)?;

        let batches: Vec<_> = rows
            .chunks(self.config.batch_size)
            .map(<[HashMap<String, ExtractedValue>]>::to_vec)
            .collect();
        let batch_count = batches.len();
        info!(
            batch_count,
            batch_size = self.config.batch_size,
            "Split into batches"
        );

        let conn = self.conn.clone();
        let table = table.to_string();
        let continue_on_error = self.config.continue_on_error;

        let max_parallelism = self.config.max_parallelism;
        let results: Vec<Result<BulkResult>> = stream::iter(batches)
            .map(|batch| {
                let conn = conn.clone();
                let table = table.clone();
                async move { Self::update_batch(&conn, &table, &batch).await }
            })
            .buffer_unordered(max_parallelism)
            .collect()
            .await;

        let mut final_result = BulkResult::new();
        for result in results {
            match result {
                Ok(r) => final_result.merge(r),
                Err(e) => {
                    if continue_on_error {
                        final_result.error_count += 1;
                        final_result.errors.push(e.to_string());
                    } else {
                        return Err(e);
                    }
                }
            }
        }

        info!(
            success = final_result.success_count,
            errors = final_result.error_count,
            "Parallel bulk update complete"
        );
        Ok(final_result)
    }

    /// Update a single batch of rows.
    async fn update_batch(
        conn: &Connection,
        table: &str,
        rows: &[HashMap<String, ExtractedValue>],
    ) -> Result<BulkResult> {
        let mut success_count = 0;

        for row in rows {
            let id = row.get("id").ok_or_else(|| {
                DataBridgeError::Query("Row must have 'id' field for update".to_string())
            })?;

            let id_value = match id {
                ExtractedValue::BigInt(v) => *v,
                ExtractedValue::Int(v) => *v as i64,
                _ => {
                    return Err(DataBridgeError::Query(
                        "id must be an integer type".to_string(),
                    ))
                }
            };

            // Build SET clause
            let mut set_parts = Vec::new();
            let mut args = PgArguments::default();
            let mut param_num = 1;

            for (col, value) in row.iter() {
                if col == "id" {
                    continue;
                }
                QueryBuilder::validate_identifier(col)?;
                set_parts.push(format!(
                    "{} = ${}",
                    QueryBuilder::quote_identifier(col),
                    param_num
                ));
                value.bind_to_arguments(&mut args)?;
                param_num += 1;
            }

            if set_parts.is_empty() {
                continue;
            }

            let sql = format!(
                "UPDATE {} SET {} WHERE id = ${}",
                QueryBuilder::quote_identifier(table),
                set_parts.join(", "),
                param_num
            );

            args.add(id_value)
                .map_err(|e| DataBridgeError::Query(format!("Failed to bind id: {}", e)))?;

            let result = sqlx::query_with(&sql, args)
                .execute(conn.pool())
                .await
                .map_err(|e| DataBridgeError::Query(format!("Update failed: {}", e)))?;

            success_count += result.rows_affected() as usize;
        }

        Ok(BulkResult {
            success_count,
            error_count: 0,
            errors: Vec::new(),
        })
    }

    /// Perform parallel bulk delete.
    ///
    /// Deletes rows by ID in parallel batches.
    #[instrument(skip(self, ids), fields(table = %table, total_ids = ids.len()))]
    pub async fn delete_parallel(&self, table: &str, ids: &[i64]) -> Result<BulkResult> {
        if ids.is_empty() {
            return Ok(BulkResult::new());
        }

        info!("Starting parallel bulk delete");
        QueryBuilder::validate_identifier(table)?;

        let batches: Vec<_> = ids
            .chunks(self.config.batch_size)
            .map(<[i64]>::to_vec)
            .collect();
        let batch_count = batches.len();
        info!(
            batch_count,
            batch_size = self.config.batch_size,
            "Split into batches"
        );

        let conn = self.conn.clone();
        let table = table.to_string();
        let continue_on_error = self.config.continue_on_error;

        let max_parallelism = self.config.max_parallelism;
        let results: Vec<Result<BulkResult>> = stream::iter(batches)
            .map(|batch| {
                let conn = conn.clone();
                let table = table.clone();
                async move { Self::delete_batch(&conn, &table, &batch).await }
            })
            .buffer_unordered(max_parallelism)
            .collect()
            .await;

        let mut final_result = BulkResult::new();
        for result in results {
            match result {
                Ok(r) => final_result.merge(r),
                Err(e) => {
                    if continue_on_error {
                        final_result.error_count += 1;
                        final_result.errors.push(e.to_string());
                    } else {
                        return Err(e);
                    }
                }
            }
        }

        info!(
            success = final_result.success_count,
            errors = final_result.error_count,
            "Parallel bulk delete complete"
        );
        Ok(final_result)
    }

    /// Delete a single batch of IDs.
    async fn delete_batch(conn: &Connection, table: &str, ids: &[i64]) -> Result<BulkResult> {
        if ids.is_empty() {
            return Ok(BulkResult::new());
        }

        // Use DELETE ... WHERE id = ANY($1) for efficient batch delete
        let sql = format!(
            "DELETE FROM {} WHERE id = ANY($1)",
            QueryBuilder::quote_identifier(table)
        );

        let result = sqlx::query(&sql)
            .bind(ids)
            .execute(conn.pool())
            .await
            .map_err(|e| DataBridgeError::Query(format!("Batch delete failed: {}", e)))?;

        Ok(BulkResult {
            success_count: result.rows_affected() as usize,
            error_count: 0,
            errors: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bulk_config_default() {
        let config = BulkConfig::default();
        assert_eq!(config.batch_size, 1000);
        assert!(config.max_parallelism > 0);
        assert!(!config.continue_on_error);
    }

    #[test]
    fn test_bulk_config_builder() {
        let config = BulkConfig::new()
            .batch_size(500)
            .max_parallelism(4)
            .continue_on_error(true);

        assert_eq!(config.batch_size, 500);
        assert_eq!(config.max_parallelism, 4);
        assert!(config.continue_on_error);
    }

    #[test]
    fn test_bulk_config_min_values() {
        let config = BulkConfig::new().batch_size(0).max_parallelism(0);

        assert_eq!(config.batch_size, 1);
        assert_eq!(config.max_parallelism, 1);
    }

    #[test]
    fn test_bulk_result_merge() {
        let mut r1 = BulkResult {
            success_count: 10,
            error_count: 1,
            errors: vec!["error1".to_string()],
        };

        let r2 = BulkResult {
            success_count: 20,
            error_count: 2,
            errors: vec!["error2".to_string(), "error3".to_string()],
        };

        r1.merge(r2);

        assert_eq!(r1.success_count, 30);
        assert_eq!(r1.error_count, 3);
        assert_eq!(r1.errors.len(), 3);
    }
}
