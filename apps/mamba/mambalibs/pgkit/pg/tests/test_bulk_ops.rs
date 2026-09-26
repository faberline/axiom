//! Integration tests for parallel bulk operations.
//!
//! Tests BulkExecutor for parallel insert, update, and delete operations
//! with various configurations and edge cases.
//!
//! Run with: cargo test -p pg --test test_bulk_ops

use pg::{BulkConfig, BulkExecutor, Connection, ExtractedValue, PoolConfig};
use std::collections::HashMap;

fn get_database_url() -> Option<String> {
    match std::env::var("DATABASE_URL").or_else(|_| std::env::var("POSTGRES_URL")) {
        Ok(uri) => Some(uri),
        Err(_) => {
            eprintln!(
                "skipping PostgreSQL bulk operation integration test: set DATABASE_URL or POSTGRES_URL"
            );
            None
        }
    }
}

macro_rules! database_url {
    () => {
        match get_database_url() {
            Some(uri) => uri,
            None => return Ok(()),
        }
    };
}

async fn cleanup_table(pool: &sqlx::PgPool, table: &str) -> Result<(), sqlx::Error> {
    sqlx::query(&format!("DROP TABLE IF EXISTS {} CASCADE", table))
        .execute(pool)
        .await?;
    Ok(())
}

// =============================================================================
// BulkConfig Unit Tests
// =============================================================================

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
fn test_bulk_config_min_batch_size() {
    let config = BulkConfig::new().batch_size(0);
    assert_eq!(config.batch_size, 1); // Should enforce minimum of 1
}

#[test]
fn test_bulk_config_min_parallelism() {
    let config = BulkConfig::new().max_parallelism(0);
    assert_eq!(config.max_parallelism, 1); // Should enforce minimum of 1
}

// =============================================================================
// Bulk Insert Tests
// =============================================================================

#[tokio::test]
async fn test_bulk_insert_empty() -> Result<(), Box<dyn std::error::Error>> {
    let uri = database_url!();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_bulk_insert_empty";

    cleanup_table(pool, table).await?;
    sqlx::query(&format!(
        "CREATE TABLE {} (id SERIAL PRIMARY KEY, name TEXT)",
        table
    ))
    .execute(pool)
    .await?;

    let executor = BulkExecutor::new(&conn, BulkConfig::default());
    let rows: Vec<HashMap<String, ExtractedValue>> = vec![];

    let result = executor.insert_parallel(table, &rows).await?;

    assert_eq!(result.success_count, 0);
    assert_eq!(result.error_count, 0);

    cleanup_table(pool, table).await?;
    Ok(())
}

#[tokio::test]
async fn test_bulk_insert_single_row() -> Result<(), Box<dyn std::error::Error>> {
    let uri = database_url!();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_bulk_insert_single";

    cleanup_table(pool, table).await?;
    sqlx::query(&format!(
        "CREATE TABLE {} (id SERIAL PRIMARY KEY, name TEXT, value INTEGER)",
        table
    ))
    .execute(pool)
    .await?;

    let executor = BulkExecutor::new(&conn, BulkConfig::default());

    let mut row = HashMap::new();
    row.insert(
        "name".to_string(),
        ExtractedValue::String("test".to_string()),
    );
    row.insert("value".to_string(), ExtractedValue::Int(42));

    let result = executor.insert_parallel(table, &[row]).await?;

    assert_eq!(result.success_count, 1);
    assert_eq!(result.error_count, 0);

    // Verify data
    let count: (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM {}", table))
        .fetch_one(pool)
        .await?;
    assert_eq!(count.0, 1);

    cleanup_table(pool, table).await?;
    Ok(())
}

#[tokio::test]
async fn test_bulk_insert_multiple_rows() -> Result<(), Box<dyn std::error::Error>> {
    let uri = database_url!();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_bulk_insert_multi";

    cleanup_table(pool, table).await?;
    sqlx::query(&format!(
        "CREATE TABLE {} (id SERIAL PRIMARY KEY, name TEXT, value INTEGER)",
        table
    ))
    .execute(pool)
    .await?;

    let executor = BulkExecutor::new(&conn, BulkConfig::new().batch_size(10));

    let rows: Vec<HashMap<String, ExtractedValue>> = (0..100)
        .map(|i| {
            let mut row = HashMap::new();
            row.insert(
                "name".to_string(),
                ExtractedValue::String(format!("item_{}", i)),
            );
            row.insert("value".to_string(), ExtractedValue::Int(i));
            row
        })
        .collect();

    let result = executor.insert_parallel(table, &rows).await?;

    assert_eq!(result.success_count, 100);
    assert_eq!(result.error_count, 0);

    // Verify data
    let count: (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM {}", table))
        .fetch_one(pool)
        .await?;
    assert_eq!(count.0, 100);

    cleanup_table(pool, table).await?;
    Ok(())
}

#[tokio::test]
async fn test_bulk_insert_large_batch() -> Result<(), Box<dyn std::error::Error>> {
    let uri = database_url!();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_bulk_insert_large";

    cleanup_table(pool, table).await?;
    sqlx::query(&format!(
        "CREATE TABLE {} (id SERIAL PRIMARY KEY, name TEXT, value INTEGER)",
        table
    ))
    .execute(pool)
    .await?;

    // Test with 1000 rows, batch_size 100 = 10 batches
    let executor = BulkExecutor::new(&conn, BulkConfig::new().batch_size(100));

    let rows: Vec<HashMap<String, ExtractedValue>> = (0..1000)
        .map(|i| {
            let mut row = HashMap::new();
            row.insert(
                "name".to_string(),
                ExtractedValue::String(format!("item_{}", i)),
            );
            row.insert("value".to_string(), ExtractedValue::Int(i));
            row
        })
        .collect();

    let result = executor.insert_parallel(table, &rows).await?;

    assert_eq!(result.success_count, 1000);
    assert_eq!(result.error_count, 0);

    cleanup_table(pool, table).await?;
    Ok(())
}

#[tokio::test]
async fn test_bulk_insert_with_nulls() -> Result<(), Box<dyn std::error::Error>> {
    let uri = database_url!();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_bulk_insert_null";

    cleanup_table(pool, table).await?;
    sqlx::query(&format!(
        "CREATE TABLE {} (id SERIAL PRIMARY KEY, name TEXT, value INTEGER)",
        table
    ))
    .execute(pool)
    .await?;

    let executor = BulkExecutor::new(&conn, BulkConfig::default());

    let mut row1 = HashMap::new();
    row1.insert(
        "name".to_string(),
        ExtractedValue::String("with_value".to_string()),
    );
    row1.insert("value".to_string(), ExtractedValue::Int(42));

    let mut row2 = HashMap::new();
    row2.insert(
        "name".to_string(),
        ExtractedValue::String("null_value".to_string()),
    );
    row2.insert("value".to_string(), ExtractedValue::Null);

    let result = executor.insert_parallel(table, &[row1, row2]).await?;

    assert_eq!(result.success_count, 2);

    // Verify NULL was inserted correctly
    let count: (i64,) = sqlx::query_as(&format!(
        "SELECT COUNT(*) FROM {} WHERE value IS NULL",
        table
    ))
    .fetch_one(pool)
    .await?;
    assert_eq!(count.0, 1);

    cleanup_table(pool, table).await?;
    Ok(())
}

#[tokio::test]
async fn test_bulk_insert_various_types() -> Result<(), Box<dyn std::error::Error>> {
    let uri = database_url!();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_bulk_insert_types";

    cleanup_table(pool, table).await?;
    sqlx::query(&format!(
        "CREATE TABLE {} (
            id SERIAL PRIMARY KEY,
            bool_val BOOLEAN,
            int_val INTEGER,
            bigint_val BIGINT,
            float_val DOUBLE PRECISION,
            text_val TEXT
        )",
        table
    ))
    .execute(pool)
    .await?;

    let executor = BulkExecutor::new(&conn, BulkConfig::default());

    let mut row = HashMap::new();
    row.insert("bool_val".to_string(), ExtractedValue::Bool(true));
    row.insert("int_val".to_string(), ExtractedValue::Int(42));
    row.insert("bigint_val".to_string(), ExtractedValue::BigInt(9999999999));
    row.insert("float_val".to_string(), ExtractedValue::Double(3.14159));
    row.insert(
        "text_val".to_string(),
        ExtractedValue::String("test".to_string()),
    );

    let result = executor.insert_parallel(table, &[row]).await?;

    assert_eq!(result.success_count, 1);

    cleanup_table(pool, table).await?;
    Ok(())
}

// =============================================================================
// Bulk Update Tests
// =============================================================================

#[tokio::test]
async fn test_bulk_update_empty() -> Result<(), Box<dyn std::error::Error>> {
    let uri = database_url!();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_bulk_update_empty";

    cleanup_table(pool, table).await?;
    sqlx::query(&format!(
        "CREATE TABLE {} (id SERIAL PRIMARY KEY, name TEXT)",
        table
    ))
    .execute(pool)
    .await?;

    let executor = BulkExecutor::new(&conn, BulkConfig::default());
    let rows: Vec<HashMap<String, ExtractedValue>> = vec![];

    let result = executor.update_parallel(table, &rows).await?;

    assert_eq!(result.success_count, 0);
    assert_eq!(result.error_count, 0);

    cleanup_table(pool, table).await?;
    Ok(())
}

#[tokio::test]
async fn test_bulk_update_single_row() -> Result<(), Box<dyn std::error::Error>> {
    let uri = database_url!();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_bulk_update_single";

    cleanup_table(pool, table).await?;
    sqlx::query(&format!(
        "CREATE TABLE {} (id SERIAL PRIMARY KEY, name TEXT, value INTEGER)",
        table
    ))
    .execute(pool)
    .await?;

    // Insert initial data
    sqlx::query(&format!(
        "INSERT INTO {} (name, value) VALUES ('original', 1)",
        table
    ))
    .execute(pool)
    .await?;

    let executor = BulkExecutor::new(&conn, BulkConfig::default());

    let mut update = HashMap::new();
    update.insert("id".to_string(), ExtractedValue::Int(1));
    update.insert(
        "name".to_string(),
        ExtractedValue::String("updated".to_string()),
    );
    update.insert("value".to_string(), ExtractedValue::Int(999));

    let result = executor.update_parallel(table, &[update]).await?;

    assert_eq!(result.success_count, 1);

    // Verify update
    let row: (String, i32) =
        sqlx::query_as(&format!("SELECT name, value FROM {} WHERE id = 1", table))
            .fetch_one(pool)
            .await?;
    assert_eq!(row.0, "updated");
    assert_eq!(row.1, 999);

    cleanup_table(pool, table).await?;
    Ok(())
}

#[tokio::test]
async fn test_bulk_update_multiple_rows() -> Result<(), Box<dyn std::error::Error>> {
    let uri = database_url!();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_bulk_update_multi";

    cleanup_table(pool, table).await?;
    sqlx::query(&format!(
        "CREATE TABLE {} (id SERIAL PRIMARY KEY, name TEXT, value INTEGER)",
        table
    ))
    .execute(pool)
    .await?;

    // Insert initial data
    for i in 1..=10 {
        sqlx::query(&format!(
            "INSERT INTO {} (name, value) VALUES ($1, $2)",
            table
        ))
        .bind(format!("item_{}", i))
        .bind(i)
        .execute(pool)
        .await?;
    }

    let executor = BulkExecutor::new(&conn, BulkConfig::new().batch_size(3));

    // Update all rows
    let updates: Vec<HashMap<String, ExtractedValue>> = (1..=10)
        .map(|i| {
            let mut row = HashMap::new();
            row.insert("id".to_string(), ExtractedValue::Int(i));
            row.insert("value".to_string(), ExtractedValue::Int(i * 100));
            row
        })
        .collect();

    let result = executor.update_parallel(table, &updates).await?;

    assert_eq!(result.success_count, 10);

    // Verify updates
    let sum: (i64,) = sqlx::query_as(&format!("SELECT SUM(value) FROM {}", table))
        .fetch_one(pool)
        .await?;
    assert_eq!(sum.0, (1..=10).map(|i| i * 100).sum::<i64>());

    cleanup_table(pool, table).await?;
    Ok(())
}

#[tokio::test]
async fn test_bulk_update_nonexistent_id() -> Result<(), Box<dyn std::error::Error>> {
    let uri = database_url!();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_bulk_update_nonexist";

    cleanup_table(pool, table).await?;
    sqlx::query(&format!(
        "CREATE TABLE {} (id SERIAL PRIMARY KEY, name TEXT)",
        table
    ))
    .execute(pool)
    .await?;

    let executor = BulkExecutor::new(&conn, BulkConfig::default());

    let mut update = HashMap::new();
    update.insert("id".to_string(), ExtractedValue::Int(9999)); // doesn't exist
    update.insert(
        "name".to_string(),
        ExtractedValue::String("ghost".to_string()),
    );

    let result = executor.update_parallel(table, &[update]).await?;

    // Should succeed but affect 0 rows
    assert_eq!(result.success_count, 0);

    cleanup_table(pool, table).await?;
    Ok(())
}

// =============================================================================
// Bulk Delete Tests
// =============================================================================

#[tokio::test]
async fn test_bulk_delete_empty() -> Result<(), Box<dyn std::error::Error>> {
    let uri = database_url!();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_bulk_delete_empty";

    cleanup_table(pool, table).await?;
    sqlx::query(&format!(
        "CREATE TABLE {} (id SERIAL PRIMARY KEY, name TEXT)",
        table
    ))
    .execute(pool)
    .await?;

    let executor = BulkExecutor::new(&conn, BulkConfig::default());
    let ids: Vec<i64> = vec![];

    let result = executor.delete_parallel(table, &ids).await?;

    assert_eq!(result.success_count, 0);
    assert_eq!(result.error_count, 0);

    cleanup_table(pool, table).await?;
    Ok(())
}

#[tokio::test]
async fn test_bulk_delete_single() -> Result<(), Box<dyn std::error::Error>> {
    let uri = database_url!();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_bulk_delete_single";

    cleanup_table(pool, table).await?;
    sqlx::query(&format!(
        "CREATE TABLE {} (id SERIAL PRIMARY KEY, name TEXT)",
        table
    ))
    .execute(pool)
    .await?;

    sqlx::query(&format!("INSERT INTO {} (name) VALUES ('test')", table))
        .execute(pool)
        .await?;

    let executor = BulkExecutor::new(&conn, BulkConfig::default());
    let result = executor.delete_parallel(table, &[1]).await?;

    assert_eq!(result.success_count, 1);

    // Verify deletion
    let count: (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM {}", table))
        .fetch_one(pool)
        .await?;
    assert_eq!(count.0, 0);

    cleanup_table(pool, table).await?;
    Ok(())
}

#[tokio::test]
async fn test_bulk_delete_multiple() -> Result<(), Box<dyn std::error::Error>> {
    let uri = database_url!();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_bulk_delete_multi";

    cleanup_table(pool, table).await?;
    sqlx::query(&format!(
        "CREATE TABLE {} (id SERIAL PRIMARY KEY, name TEXT)",
        table
    ))
    .execute(pool)
    .await?;

    // Insert 100 rows
    for i in 0..100 {
        sqlx::query(&format!("INSERT INTO {} (name) VALUES ($1)", table))
            .bind(format!("item_{}", i))
            .execute(pool)
            .await?;
    }

    let executor = BulkExecutor::new(&conn, BulkConfig::new().batch_size(10));

    // Delete IDs 1-50
    let ids: Vec<i64> = (1..=50).collect();
    let result = executor.delete_parallel(table, &ids).await?;

    assert_eq!(result.success_count, 50);

    // Verify remaining count
    let count: (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM {}", table))
        .fetch_one(pool)
        .await?;
    assert_eq!(count.0, 50);

    cleanup_table(pool, table).await?;
    Ok(())
}

#[tokio::test]
async fn test_bulk_delete_nonexistent_ids() -> Result<(), Box<dyn std::error::Error>> {
    let uri = database_url!();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_bulk_delete_nonexist";

    cleanup_table(pool, table).await?;
    sqlx::query(&format!(
        "CREATE TABLE {} (id SERIAL PRIMARY KEY, name TEXT)",
        table
    ))
    .execute(pool)
    .await?;

    sqlx::query(&format!("INSERT INTO {} (name) VALUES ('test')", table))
        .execute(pool)
        .await?;

    let executor = BulkExecutor::new(&conn, BulkConfig::default());

    // Try to delete non-existent IDs
    let result = executor.delete_parallel(table, &[9999, 9998, 9997]).await?;

    assert_eq!(result.success_count, 0);

    // Verify original row still exists
    let count: (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM {}", table))
        .fetch_one(pool)
        .await?;
    assert_eq!(count.0, 1);

    cleanup_table(pool, table).await?;
    Ok(())
}

// =============================================================================
// Error Handling Tests
// =============================================================================

#[tokio::test]
async fn test_bulk_insert_continue_on_error() -> Result<(), Box<dyn std::error::Error>> {
    let uri = database_url!();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_bulk_continue_error";

    cleanup_table(pool, table).await?;
    sqlx::query(&format!(
        "CREATE TABLE {} (id SERIAL PRIMARY KEY, name TEXT UNIQUE NOT NULL)",
        table
    ))
    .execute(pool)
    .await?;

    // Insert one row first
    sqlx::query(&format!("INSERT INTO {} (name) VALUES ('existing')", table))
        .execute(pool)
        .await?;

    let executor = BulkExecutor::new(
        &conn,
        BulkConfig::new().batch_size(1).continue_on_error(true),
    );

    // Try to insert duplicates - some will fail due to unique constraint
    let rows: Vec<HashMap<String, ExtractedValue>> = vec![
        {
            let mut row = HashMap::new();
            row.insert(
                "name".to_string(),
                ExtractedValue::String("new1".to_string()),
            );
            row
        },
        {
            let mut row = HashMap::new();
            row.insert(
                "name".to_string(),
                ExtractedValue::String("existing".to_string()),
            ); // Will fail
            row
        },
        {
            let mut row = HashMap::new();
            row.insert(
                "name".to_string(),
                ExtractedValue::String("new2".to_string()),
            );
            row
        },
    ];

    let result = executor.insert_parallel(table, &rows).await?;

    // Should have some successes and one error
    assert!(result.success_count >= 1);
    assert!(result.error_count >= 1);
    assert!(!result.errors.is_empty());

    cleanup_table(pool, table).await?;
    Ok(())
}

#[tokio::test]
async fn test_bulk_insert_fail_fast() -> Result<(), Box<dyn std::error::Error>> {
    let uri = database_url!();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_bulk_fail_fast";

    cleanup_table(pool, table).await?;
    sqlx::query(&format!(
        "CREATE TABLE {} (id SERIAL PRIMARY KEY, name TEXT UNIQUE NOT NULL)",
        table
    ))
    .execute(pool)
    .await?;

    // Insert one row first
    sqlx::query(&format!("INSERT INTO {} (name) VALUES ('existing')", table))
        .execute(pool)
        .await?;

    let executor = BulkExecutor::new(
        &conn,
        BulkConfig::new().batch_size(1).continue_on_error(false), // fail fast
    );

    // Try to insert duplicate - should fail immediately
    let mut row = HashMap::new();
    row.insert(
        "name".to_string(),
        ExtractedValue::String("existing".to_string()),
    );

    let result = executor.insert_parallel(table, &[row]).await;

    // Should return error
    assert!(result.is_err());

    cleanup_table(pool, table).await?;
    Ok(())
}

// =============================================================================
// Parallelism Tests
// =============================================================================

#[tokio::test]
async fn test_bulk_insert_parallelism() -> Result<(), Box<dyn std::error::Error>> {
    let uri = database_url!();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_bulk_parallel";

    cleanup_table(pool, table).await?;
    sqlx::query(&format!(
        "CREATE TABLE {} (id SERIAL PRIMARY KEY, batch_id INTEGER, item_idx INTEGER)",
        table
    ))
    .execute(pool)
    .await?;

    // Use small batch size to force many parallel batches
    let executor = BulkExecutor::new(&conn, BulkConfig::new().batch_size(10).max_parallelism(4));

    // Insert 100 rows
    let rows: Vec<HashMap<String, ExtractedValue>> = (0..100)
        .map(|i| {
            let mut row = HashMap::new();
            row.insert("batch_id".to_string(), ExtractedValue::Int(i / 10));
            row.insert("item_idx".to_string(), ExtractedValue::Int(i % 10));
            row
        })
        .collect();

    let result = executor.insert_parallel(table, &rows).await?;

    assert_eq!(result.success_count, 100);

    // Verify all data is there
    let count: (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM {}", table))
        .fetch_one(pool)
        .await?;
    assert_eq!(count.0, 100);

    cleanup_table(pool, table).await?;
    Ok(())
}

#[tokio::test]
async fn test_bulk_concurrent_operations() -> Result<(), Box<dyn std::error::Error>> {
    let uri = database_url!();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_bulk_concurrent";

    cleanup_table(pool, table).await?;
    sqlx::query(&format!(
        "CREATE TABLE {} (id SERIAL PRIMARY KEY, counter INTEGER DEFAULT 0)",
        table
    ))
    .execute(pool)
    .await?;

    // Insert initial rows
    for _ in 0..10 {
        sqlx::query(&format!("INSERT INTO {} DEFAULT VALUES", table))
            .execute(pool)
            .await?;
    }

    // Run multiple bulk operations concurrently
    let executor = BulkExecutor::new(&conn, BulkConfig::new().batch_size(5));

    // Update all rows concurrently from multiple "threads"
    let updates: Vec<HashMap<String, ExtractedValue>> = (1..=10)
        .map(|i| {
            let mut row = HashMap::new();
            row.insert("id".to_string(), ExtractedValue::Int(i));
            row.insert("counter".to_string(), ExtractedValue::Int(1));
            row
        })
        .collect();

    let result = executor.update_parallel(table, &updates).await?;

    assert_eq!(result.success_count, 10);

    cleanup_table(pool, table).await?;
    Ok(())
}

// =============================================================================
// Edge Cases
// =============================================================================

#[tokio::test]
async fn test_bulk_insert_special_characters() -> Result<(), Box<dyn std::error::Error>> {
    let uri = database_url!();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_bulk_special_chars";

    cleanup_table(pool, table).await?;
    sqlx::query(&format!(
        "CREATE TABLE {} (id SERIAL PRIMARY KEY, name TEXT)",
        table
    ))
    .execute(pool)
    .await?;

    let executor = BulkExecutor::new(&conn, BulkConfig::default());

    let special_strings = vec![
        "normal",
        "with'quote",
        "with\"double",
        "with\\backslash",
        "unicode:\u{1F600}",
        "newline\nhere",
        "tab\there",
        "", // empty string
    ];

    let rows: Vec<HashMap<String, ExtractedValue>> = special_strings
        .iter()
        .map(|s| {
            let mut row = HashMap::new();
            row.insert("name".to_string(), ExtractedValue::String(s.to_string()));
            row
        })
        .collect();

    let result = executor.insert_parallel(table, &rows).await?;

    assert_eq!(result.success_count, special_strings.len());

    // Verify all strings stored correctly
    let stored: Vec<(String,)> = sqlx::query_as(&format!("SELECT name FROM {} ORDER BY id", table))
        .fetch_all(pool)
        .await?;

    for (i, row) in stored.iter().enumerate() {
        assert_eq!(row.0, special_strings[i]);
    }

    cleanup_table(pool, table).await?;
    Ok(())
}

#[tokio::test]
async fn test_bulk_insert_exact_batch_size() -> Result<(), Box<dyn std::error::Error>> {
    let uri = database_url!();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_bulk_exact_batch";

    cleanup_table(pool, table).await?;
    sqlx::query(&format!(
        "CREATE TABLE {} (id SERIAL PRIMARY KEY, idx INTEGER)",
        table
    ))
    .execute(pool)
    .await?;

    // Insert exactly batch_size rows (boundary case)
    let batch_size = 10;
    let executor = BulkExecutor::new(&conn, BulkConfig::new().batch_size(batch_size));

    let rows: Vec<HashMap<String, ExtractedValue>> = (0..batch_size as i32)
        .map(|i| {
            let mut row = HashMap::new();
            row.insert("idx".to_string(), ExtractedValue::Int(i));
            row
        })
        .collect();

    let result = executor.insert_parallel(table, &rows).await?;

    assert_eq!(result.success_count, batch_size);

    cleanup_table(pool, table).await?;
    Ok(())
}

#[tokio::test]
async fn test_bulk_update_only_id_field() -> Result<(), Box<dyn std::error::Error>> {
    let uri = database_url!();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_bulk_update_only_id";

    cleanup_table(pool, table).await?;
    sqlx::query(&format!(
        "CREATE TABLE {} (id SERIAL PRIMARY KEY, name TEXT)",
        table
    ))
    .execute(pool)
    .await?;

    sqlx::query(&format!("INSERT INTO {} (name) VALUES ('test')", table))
        .execute(pool)
        .await?;

    let executor = BulkExecutor::new(&conn, BulkConfig::default());

    // Update with only id field (no other fields to update)
    let mut update = HashMap::new();
    update.insert("id".to_string(), ExtractedValue::Int(1));

    let result = executor.update_parallel(table, &[update]).await?;

    // Should succeed with 0 affected rows (nothing to update)
    assert_eq!(result.success_count, 0);

    cleanup_table(pool, table).await?;
    Ok(())
}
