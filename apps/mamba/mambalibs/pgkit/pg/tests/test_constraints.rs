//! Integration tests for Constraint Violation Error Handling.
//!
//! These tests require a PostgreSQL database to be running.
//! Set DATABASE_URL environment variable to customize connection.
//!
//! Run with: cargo test -p cclab-titan --test test_constraints

use pg::{Connection, PoolConfig};

/// Helper to get database URL from environment
fn get_database_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql://localhost/test_db".to_string())
}

/// Helper to cleanup test table
async fn cleanup_table(pool: &sqlx::PgPool, table_name: &str) -> Result<(), sqlx::Error> {
    sqlx::query(&format!("DROP TABLE IF EXISTS {} CASCADE", table_name))
        .execute(pool)
        .await?;
    Ok(())
}

// =============================================================================
// Primary Key Violation Tests
// =============================================================================

#[tokio::test]
async fn test_primary_key_violation() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_pk_violation";

    cleanup_table(pool, table).await?;

    sqlx::query(&format!(
        "CREATE TABLE {} (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL
        )",
        table
    ))
    .execute(pool)
    .await?;

    // Insert first record
    sqlx::query(&format!("INSERT INTO {} (id, name) VALUES ($1, $2)", table))
        .bind(1)
        .bind("Alice")
        .execute(pool)
        .await?;

    // Attempt duplicate primary key
    let result = sqlx::query(&format!("INSERT INTO {} (id, name) VALUES ($1, $2)", table))
        .bind(1)
        .bind("Bob")
        .execute(pool)
        .await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    let err_str = err.to_string();
    assert!(
        err_str.contains("duplicate key") || err_str.contains("unique constraint"),
        "Expected duplicate key error, got: {}",
        err_str
    );

    cleanup_table(pool, table).await?;
    Ok(())
}

#[tokio::test]
async fn test_serial_pk_auto_increment() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_serial_pk";

    cleanup_table(pool, table).await?;

    sqlx::query(&format!(
        "CREATE TABLE {} (
            id SERIAL PRIMARY KEY,
            name TEXT NOT NULL
        )",
        table
    ))
    .execute(pool)
    .await?;

    // Insert records without specifying id
    let row1: (i32,) = sqlx::query_as(&format!(
        "INSERT INTO {} (name) VALUES ($1) RETURNING id",
        table
    ))
    .bind("Alice")
    .fetch_one(pool)
    .await?;

    let row2: (i32,) = sqlx::query_as(&format!(
        "INSERT INTO {} (name) VALUES ($1) RETURNING id",
        table
    ))
    .bind("Bob")
    .fetch_one(pool)
    .await?;

    assert!(row2.0 > row1.0);

    cleanup_table(pool, table).await?;
    Ok(())
}

// =============================================================================
// Unique Constraint Violation Tests
// =============================================================================

#[tokio::test]
async fn test_unique_constraint_violation() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_unique_violation";

    cleanup_table(pool, table).await?;

    sqlx::query(&format!(
        "CREATE TABLE {} (
            id SERIAL PRIMARY KEY,
            email TEXT NOT NULL UNIQUE
        )",
        table
    ))
    .execute(pool)
    .await?;

    // Insert first record
    sqlx::query(&format!("INSERT INTO {} (email) VALUES ($1)", table))
        .bind("alice@example.com")
        .execute(pool)
        .await?;

    // Attempt duplicate email
    let result = sqlx::query(&format!("INSERT INTO {} (email) VALUES ($1)", table))
        .bind("alice@example.com")
        .execute(pool)
        .await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    let err_str = err.to_string();
    assert!(
        err_str.contains("duplicate key") || err_str.contains("unique constraint"),
        "Expected unique constraint error, got: {}",
        err_str
    );

    cleanup_table(pool, table).await?;
    Ok(())
}

#[tokio::test]
async fn test_unique_constraint_null_values() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_unique_null";

    cleanup_table(pool, table).await?;

    sqlx::query(&format!(
        "CREATE TABLE {} (
            id SERIAL PRIMARY KEY,
            optional_code TEXT UNIQUE
        )",
        table
    ))
    .execute(pool)
    .await?;

    // Multiple NULL values should be allowed (NULL != NULL)
    sqlx::query(&format!(
        "INSERT INTO {} (optional_code) VALUES (NULL), (NULL)",
        table
    ))
    .execute(pool)
    .await?;

    let count: (i64,) = sqlx::query_as(&format!(
        "SELECT COUNT(*) FROM {} WHERE optional_code IS NULL",
        table
    ))
    .fetch_one(pool)
    .await?;

    assert_eq!(count.0, 2);

    cleanup_table(pool, table).await?;
    Ok(())
}

#[tokio::test]
async fn test_composite_unique_constraint() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_composite_unique";

    cleanup_table(pool, table).await?;

    sqlx::query(&format!(
        "CREATE TABLE {} (
            id SERIAL PRIMARY KEY,
            first_name TEXT NOT NULL,
            last_name TEXT NOT NULL,
            UNIQUE (first_name, last_name)
        )",
        table
    ))
    .execute(pool)
    .await?;

    // Insert first record
    sqlx::query(&format!(
        "INSERT INTO {} (first_name, last_name) VALUES ($1, $2)",
        table
    ))
    .bind("John")
    .bind("Doe")
    .execute(pool)
    .await?;

    // Same first name, different last name - should work
    sqlx::query(&format!(
        "INSERT INTO {} (first_name, last_name) VALUES ($1, $2)",
        table
    ))
    .bind("John")
    .bind("Smith")
    .execute(pool)
    .await?;

    // Duplicate composite key - should fail
    let result = sqlx::query(&format!(
        "INSERT INTO {} (first_name, last_name) VALUES ($1, $2)",
        table
    ))
    .bind("John")
    .bind("Doe")
    .execute(pool)
    .await;

    assert!(result.is_err());

    cleanup_table(pool, table).await?;
    Ok(())
}

// =============================================================================
// NOT NULL Constraint Violation Tests
// =============================================================================

#[tokio::test]
async fn test_not_null_violation() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_not_null";

    cleanup_table(pool, table).await?;

    sqlx::query(&format!(
        "CREATE TABLE {} (
            id SERIAL PRIMARY KEY,
            name TEXT NOT NULL,
            email TEXT NOT NULL
        )",
        table
    ))
    .execute(pool)
    .await?;

    // Attempt to insert NULL into NOT NULL column
    let result = sqlx::query(&format!(
        "INSERT INTO {} (name, email) VALUES ($1, $2)",
        table
    ))
    .bind(Option::<String>::None)
    .bind("test@example.com")
    .execute(pool)
    .await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    let err_str = err.to_string();
    assert!(
        err_str.contains("null value") || err_str.contains("NOT NULL"),
        "Expected NOT NULL violation, got: {}",
        err_str
    );

    cleanup_table(pool, table).await?;
    Ok(())
}

#[tokio::test]
async fn test_not_null_with_default() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_not_null_default";

    cleanup_table(pool, table).await?;

    sqlx::query(&format!(
        "CREATE TABLE {} (
            id SERIAL PRIMARY KEY,
            name TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending'
        )",
        table
    ))
    .execute(pool)
    .await?;

    // Insert without specifying status - should use default
    let row: (String,) = sqlx::query_as(&format!(
        "INSERT INTO {} (name) VALUES ($1) RETURNING status",
        table
    ))
    .bind("Alice")
    .fetch_one(pool)
    .await?;

    assert_eq!(row.0, "pending");

    cleanup_table(pool, table).await?;
    Ok(())
}

// =============================================================================
// Foreign Key Constraint Violation Tests
// =============================================================================

#[tokio::test]
async fn test_foreign_key_violation_insert() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let parent_table = "test_fk_parent";
    let child_table = "test_fk_child";

    cleanup_table(pool, child_table).await?;
    cleanup_table(pool, parent_table).await?;

    // Create parent table
    sqlx::query(&format!(
        "CREATE TABLE {} (
            id SERIAL PRIMARY KEY,
            name TEXT NOT NULL
        )",
        parent_table
    ))
    .execute(pool)
    .await?;

    // Create child table with foreign key
    sqlx::query(&format!(
        "CREATE TABLE {} (
            id SERIAL PRIMARY KEY,
            parent_id INTEGER NOT NULL REFERENCES {}(id),
            data TEXT NOT NULL
        )",
        child_table, parent_table
    ))
    .execute(pool)
    .await?;

    // Attempt to insert with non-existent parent_id
    let result = sqlx::query(&format!(
        "INSERT INTO {} (parent_id, data) VALUES ($1, $2)",
        child_table
    ))
    .bind(999)
    .bind("orphan data")
    .execute(pool)
    .await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    let err_str = err.to_string();
    assert!(
        err_str.contains("foreign key") || err_str.contains("violates"),
        "Expected foreign key violation, got: {}",
        err_str
    );

    cleanup_table(pool, child_table).await?;
    cleanup_table(pool, parent_table).await?;
    Ok(())
}

#[tokio::test]
async fn test_foreign_key_violation_delete() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let parent_table = "test_fk_del_parent";
    let child_table = "test_fk_del_child";

    cleanup_table(pool, child_table).await?;
    cleanup_table(pool, parent_table).await?;

    // Create parent table
    sqlx::query(&format!(
        "CREATE TABLE {} (
            id SERIAL PRIMARY KEY,
            name TEXT NOT NULL
        )",
        parent_table
    ))
    .execute(pool)
    .await?;

    // Create child table with RESTRICT on delete
    sqlx::query(&format!(
        "CREATE TABLE {} (
            id SERIAL PRIMARY KEY,
            parent_id INTEGER NOT NULL REFERENCES {}(id) ON DELETE RESTRICT,
            data TEXT NOT NULL
        )",
        child_table, parent_table
    ))
    .execute(pool)
    .await?;

    // Insert parent and child
    let parent: (i32,) = sqlx::query_as(&format!(
        "INSERT INTO {} (name) VALUES ($1) RETURNING id",
        parent_table
    ))
    .bind("Parent")
    .fetch_one(pool)
    .await?;

    sqlx::query(&format!(
        "INSERT INTO {} (parent_id, data) VALUES ($1, $2)",
        child_table
    ))
    .bind(parent.0)
    .bind("Child data")
    .execute(pool)
    .await?;

    // Attempt to delete parent with existing child - should fail
    let result = sqlx::query(&format!("DELETE FROM {} WHERE id = $1", parent_table))
        .bind(parent.0)
        .execute(pool)
        .await;

    assert!(result.is_err());

    cleanup_table(pool, child_table).await?;
    cleanup_table(pool, parent_table).await?;
    Ok(())
}

// =============================================================================
// Check Constraint Violation Tests
// =============================================================================

#[tokio::test]
async fn test_check_constraint_violation() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_check_constraint";

    cleanup_table(pool, table).await?;

    sqlx::query(&format!(
        "CREATE TABLE {} (
            id SERIAL PRIMARY KEY,
            age INTEGER NOT NULL CHECK (age >= 0 AND age <= 150),
            price NUMERIC(10,2) CHECK (price > 0)
        )",
        table
    ))
    .execute(pool)
    .await?;

    // Valid insert
    sqlx::query(&format!(
        "INSERT INTO {} (age, price) VALUES ($1, $2)",
        table
    ))
    .bind(25)
    .bind(19.99f64)
    .execute(pool)
    .await?;

    // Violate age check (negative)
    let result = sqlx::query(&format!(
        "INSERT INTO {} (age, price) VALUES ($1, $2)",
        table
    ))
    .bind(-5)
    .bind(19.99f64)
    .execute(pool)
    .await;

    assert!(result.is_err());
    let err_str = result.unwrap_err().to_string();
    assert!(
        err_str.contains("check") || err_str.contains("violates"),
        "Expected check constraint error, got: {}",
        err_str
    );

    // Violate age check (too high)
    let result = sqlx::query(&format!(
        "INSERT INTO {} (age, price) VALUES ($1, $2)",
        table
    ))
    .bind(200)
    .bind(19.99f64)
    .execute(pool)
    .await;

    assert!(result.is_err());

    // Violate price check (zero)
    let result = sqlx::query(&format!(
        "INSERT INTO {} (age, price) VALUES ($1, $2)",
        table
    ))
    .bind(25)
    .bind(0.0f64)
    .execute(pool)
    .await;

    assert!(result.is_err());

    cleanup_table(pool, table).await?;
    Ok(())
}

#[tokio::test]
async fn test_check_constraint_with_expression() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_check_expr";

    cleanup_table(pool, table).await?;

    sqlx::query(&format!(
        "CREATE TABLE {} (
            id SERIAL PRIMARY KEY,
            start_date DATE NOT NULL,
            end_date DATE NOT NULL,
            CHECK (end_date > start_date)
        )",
        table
    ))
    .execute(pool)
    .await?;

    // Valid: end_date > start_date
    sqlx::query(&format!(
        "INSERT INTO {} (start_date, end_date) VALUES ($1::date, $2::date)",
        table
    ))
    .bind("2024-01-01")
    .bind("2024-12-31")
    .execute(pool)
    .await?;

    // Invalid: end_date <= start_date
    let result = sqlx::query(&format!(
        "INSERT INTO {} (start_date, end_date) VALUES ($1::date, $2::date)",
        table
    ))
    .bind("2024-06-01")
    .bind("2024-01-01")
    .execute(pool)
    .await;

    assert!(result.is_err());

    cleanup_table(pool, table).await?;
    Ok(())
}

// =============================================================================
// Exclusion Constraint Tests (PostgreSQL specific)
// =============================================================================

#[tokio::test]
async fn test_exclusion_constraint() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_exclusion";

    cleanup_table(pool, table).await?;

    // Enable btree_gist extension for range exclusion
    sqlx::query("CREATE EXTENSION IF NOT EXISTS btree_gist")
        .execute(pool)
        .await?;

    sqlx::query(&format!(
        "CREATE TABLE {} (
            id SERIAL PRIMARY KEY,
            room_id INTEGER NOT NULL,
            time_range TSRANGE NOT NULL,
            EXCLUDE USING GIST (room_id WITH =, time_range WITH &&)
        )",
        table
    ))
    .execute(pool)
    .await?;

    // Book room 1 from 10:00 to 12:00
    sqlx::query(&format!(
        "INSERT INTO {} (room_id, time_range) VALUES ($1, $2::tsrange)",
        table
    ))
    .bind(1)
    .bind("[2024-01-15 10:00:00, 2024-01-15 12:00:00)")
    .execute(pool)
    .await?;

    // Attempt overlapping booking - should fail
    let result = sqlx::query(&format!(
        "INSERT INTO {} (room_id, time_range) VALUES ($1, $2::tsrange)",
        table
    ))
    .bind(1)
    .bind("[2024-01-15 11:00:00, 2024-01-15 13:00:00)")
    .execute(pool)
    .await;

    assert!(result.is_err());

    // Non-overlapping booking for same room - should succeed
    sqlx::query(&format!(
        "INSERT INTO {} (room_id, time_range) VALUES ($1, $2::tsrange)",
        table
    ))
    .bind(1)
    .bind("[2024-01-15 14:00:00, 2024-01-15 16:00:00)")
    .execute(pool)
    .await?;

    // Overlapping time but different room - should succeed
    sqlx::query(&format!(
        "INSERT INTO {} (room_id, time_range) VALUES ($1, $2::tsrange)",
        table
    ))
    .bind(2)
    .bind("[2024-01-15 10:00:00, 2024-01-15 12:00:00)")
    .execute(pool)
    .await?;

    cleanup_table(pool, table).await?;
    Ok(())
}

// =============================================================================
// Data Type Constraint Tests
// =============================================================================

#[tokio::test]
async fn test_numeric_precision_constraint() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_numeric_precision";

    cleanup_table(pool, table).await?;

    sqlx::query(&format!(
        "CREATE TABLE {} (
            id SERIAL PRIMARY KEY,
            amount NUMERIC(5,2) NOT NULL
        )",
        table
    ))
    .execute(pool)
    .await?;

    // Valid: within precision
    sqlx::query(&format!("INSERT INTO {} (amount) VALUES ($1)", table))
        .bind(999.99f64)
        .execute(pool)
        .await?;

    // Exceeds precision (> 999.99)
    let result = sqlx::query(&format!("INSERT INTO {} (amount) VALUES ($1)", table))
        .bind(10000.00f64)
        .execute(pool)
        .await;

    assert!(result.is_err());

    cleanup_table(pool, table).await?;
    Ok(())
}

#[tokio::test]
async fn test_varchar_length_constraint() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_varchar_length";

    cleanup_table(pool, table).await?;

    sqlx::query(&format!(
        "CREATE TABLE {} (
            id SERIAL PRIMARY KEY,
            code VARCHAR(5) NOT NULL
        )",
        table
    ))
    .execute(pool)
    .await?;

    // Valid: within length
    sqlx::query(&format!("INSERT INTO {} (code) VALUES ($1)", table))
        .bind("ABC")
        .execute(pool)
        .await?;

    // Exceeds length
    let result = sqlx::query(&format!("INSERT INTO {} (code) VALUES ($1)", table))
        .bind("TOOLONGCODE")
        .execute(pool)
        .await;

    assert!(result.is_err());
    let err_str = result.unwrap_err().to_string();
    assert!(
        err_str.contains("value too long") || err_str.contains("character varying"),
        "Expected length constraint error, got: {}",
        err_str
    );

    cleanup_table(pool, table).await?;
    Ok(())
}

// =============================================================================
// Error Recovery Tests
// =============================================================================

#[tokio::test]
async fn test_constraint_error_recovery() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_error_recovery";

    cleanup_table(pool, table).await?;

    sqlx::query(&format!(
        "CREATE TABLE {} (
            id SERIAL PRIMARY KEY,
            code TEXT NOT NULL UNIQUE
        )",
        table
    ))
    .execute(pool)
    .await?;

    // Insert valid record
    sqlx::query(&format!("INSERT INTO {} (code) VALUES ($1)", table))
        .bind("CODE1")
        .execute(pool)
        .await?;

    // Attempt duplicate - should fail
    let result = sqlx::query(&format!("INSERT INTO {} (code) VALUES ($1)", table))
        .bind("CODE1")
        .execute(pool)
        .await;

    assert!(result.is_err());

    // Connection should still be usable after error
    sqlx::query(&format!("INSERT INTO {} (code) VALUES ($1)", table))
        .bind("CODE2")
        .execute(pool)
        .await?;

    let count: (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM {}", table))
        .fetch_one(pool)
        .await?;

    assert_eq!(count.0, 2);

    cleanup_table(pool, table).await?;
    Ok(())
}

#[tokio::test]
async fn test_transaction_rollback_on_constraint_error() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_txn_rollback";

    cleanup_table(pool, table).await?;

    sqlx::query(&format!(
        "CREATE TABLE {} (
            id SERIAL PRIMARY KEY,
            value INTEGER NOT NULL UNIQUE
        )",
        table
    ))
    .execute(pool)
    .await?;

    // Start transaction
    let mut tx = pool.begin().await?;

    // Insert first record
    sqlx::query(&format!("INSERT INTO {} (value) VALUES ($1)", table))
        .bind(100)
        .execute(&mut *tx)
        .await?;

    // Insert second record
    sqlx::query(&format!("INSERT INTO {} (value) VALUES ($1)", table))
        .bind(200)
        .execute(&mut *tx)
        .await?;

    // Attempt duplicate - should fail
    let result = sqlx::query(&format!("INSERT INTO {} (value) VALUES ($1)", table))
        .bind(100)
        .execute(&mut *tx)
        .await;

    assert!(result.is_err());

    // Rollback transaction
    tx.rollback().await?;

    // Verify no records were inserted
    let count: (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM {}", table))
        .fetch_one(pool)
        .await?;

    assert_eq!(count.0, 0);

    cleanup_table(pool, table).await?;
    Ok(())
}

// =============================================================================
// ERROR CASES - FK Update Violations
// =============================================================================

#[tokio::test]
async fn test_fk_update_violation_child() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let parent_table = "test_fk_update_parent";
    let child_table = "test_fk_update_child";

    cleanup_table(pool, child_table).await?;
    cleanup_table(pool, parent_table).await?;

    // Create parent table
    sqlx::query(&format!(
        "CREATE TABLE {} (id SERIAL PRIMARY KEY, name TEXT NOT NULL)",
        parent_table
    ))
    .execute(pool)
    .await?;

    // Create child table with FK
    sqlx::query(&format!(
        "CREATE TABLE {} (
            id SERIAL PRIMARY KEY,
            parent_id INTEGER NOT NULL REFERENCES {}(id),
            data TEXT NOT NULL
        )",
        child_table, parent_table
    ))
    .execute(pool)
    .await?;

    // Insert parent
    let parent: (i32,) = sqlx::query_as(&format!(
        "INSERT INTO {} (name) VALUES ($1) RETURNING id",
        parent_table
    ))
    .bind("Parent")
    .fetch_one(pool)
    .await?;

    // Insert child
    sqlx::query(&format!(
        "INSERT INTO {} (parent_id, data) VALUES ($1, $2)",
        child_table
    ))
    .bind(parent.0)
    .bind("Child data")
    .execute(pool)
    .await?;

    // Try to update child to non-existent parent - should fail
    let result = sqlx::query(&format!(
        "UPDATE {} SET parent_id = $1 WHERE parent_id = $2",
        child_table
    ))
    .bind(99999)
    .bind(parent.0)
    .execute(pool)
    .await;

    assert!(result.is_err());
    let err_str = result.unwrap_err().to_string().to_lowercase();
    assert!(
        err_str.contains("foreign key") || err_str.contains("violates"),
        "Expected FK violation, got: {}",
        err_str
    );

    cleanup_table(pool, child_table).await?;
    cleanup_table(pool, parent_table).await?;
    Ok(())
}

#[tokio::test]
async fn test_fk_update_parent_without_cascade() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let parent_table = "test_fk_upd_parent_nc";
    let child_table = "test_fk_upd_child_nc";

    cleanup_table(pool, child_table).await?;
    cleanup_table(pool, parent_table).await?;

    // Create parent with non-serial PK
    sqlx::query(&format!(
        "CREATE TABLE {} (id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
        parent_table
    ))
    .execute(pool)
    .await?;

    // Create child with FK (no ON UPDATE CASCADE)
    sqlx::query(&format!(
        "CREATE TABLE {} (
            id SERIAL PRIMARY KEY,
            parent_id INTEGER NOT NULL REFERENCES {}(id),
            data TEXT NOT NULL
        )",
        child_table, parent_table
    ))
    .execute(pool)
    .await?;

    // Insert parent and child
    sqlx::query(&format!(
        "INSERT INTO {} (id, name) VALUES ($1, $2)",
        parent_table
    ))
    .bind(100)
    .bind("Parent")
    .execute(pool)
    .await?;

    sqlx::query(&format!(
        "INSERT INTO {} (parent_id, data) VALUES ($1, $2)",
        child_table
    ))
    .bind(100)
    .bind("Child")
    .execute(pool)
    .await?;

    // Try to update parent PK - should fail without CASCADE
    let result = sqlx::query(&format!(
        "UPDATE {} SET id = $1 WHERE id = $2",
        parent_table
    ))
    .bind(200)
    .bind(100)
    .execute(pool)
    .await;

    assert!(result.is_err());

    cleanup_table(pool, child_table).await?;
    cleanup_table(pool, parent_table).await?;
    Ok(())
}

// =============================================================================
// EDGE CASES - Boundary Values for CHECK Constraints
// =============================================================================

#[tokio::test]
async fn test_check_constraint_boundary_values() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_check_boundary";

    cleanup_table(pool, table).await?;

    sqlx::query(&format!(
        "CREATE TABLE {} (
            id SERIAL PRIMARY KEY,
            age INTEGER NOT NULL CHECK (age >= 0 AND age <= 150),
            score NUMERIC(5,2) CHECK (score >= 0 AND score <= 100)
        )",
        table
    ))
    .execute(pool)
    .await?;

    // Test boundary: age = 0 (valid minimum)
    sqlx::query(&format!(
        "INSERT INTO {} (age, score) VALUES ($1, $2)",
        table
    ))
    .bind(0)
    .bind(50.0f64)
    .execute(pool)
    .await?;

    // Test boundary: age = 150 (valid maximum)
    sqlx::query(&format!(
        "INSERT INTO {} (age, score) VALUES ($1, $2)",
        table
    ))
    .bind(150)
    .bind(50.0f64)
    .execute(pool)
    .await?;

    // Test boundary: score = 0 (valid minimum)
    sqlx::query(&format!(
        "INSERT INTO {} (age, score) VALUES ($1, $2)",
        table
    ))
    .bind(25)
    .bind(0.0f64)
    .execute(pool)
    .await?;

    // Test boundary: score = 100 (valid maximum)
    sqlx::query(&format!(
        "INSERT INTO {} (age, score) VALUES ($1, $2)",
        table
    ))
    .bind(25)
    .bind(100.0f64)
    .execute(pool)
    .await?;

    // Test just outside boundary: age = -1
    let result = sqlx::query(&format!(
        "INSERT INTO {} (age, score) VALUES ($1, $2)",
        table
    ))
    .bind(-1)
    .bind(50.0f64)
    .execute(pool)
    .await;
    assert!(result.is_err());

    // Test just outside boundary: age = 151
    let result = sqlx::query(&format!(
        "INSERT INTO {} (age, score) VALUES ($1, $2)",
        table
    ))
    .bind(151)
    .bind(50.0f64)
    .execute(pool)
    .await;
    assert!(result.is_err());

    // Test just outside boundary: score = -0.01
    let result = sqlx::query(&format!(
        "INSERT INTO {} (age, score) VALUES ($1, $2)",
        table
    ))
    .bind(25)
    .bind(-0.01f64)
    .execute(pool)
    .await;
    assert!(result.is_err());

    // Test just outside boundary: score = 100.01
    let result = sqlx::query(&format!(
        "INSERT INTO {} (age, score) VALUES ($1, $2)",
        table
    ))
    .bind(25)
    .bind(100.01f64)
    .execute(pool)
    .await;
    assert!(result.is_err());

    cleanup_table(pool, table).await?;
    Ok(())
}

#[tokio::test]
async fn test_numeric_scale_boundary() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_numeric_scale";

    cleanup_table(pool, table).await?;

    sqlx::query(&format!(
        "CREATE TABLE {} (
            id SERIAL PRIMARY KEY,
            amount NUMERIC(10,2) NOT NULL
        )",
        table
    ))
    .execute(pool)
    .await?;

    // Insert with exact precision
    sqlx::query(&format!("INSERT INTO {} (amount) VALUES ($1)", table))
        .bind(123.45f64)
        .execute(pool)
        .await?;

    // Insert with more decimal places - should round
    sqlx::query(&format!("INSERT INTO {} (amount) VALUES ($1)", table))
        .bind(67.899f64) // Will be rounded to 67.90
        .execute(pool)
        .await?;

    // Verify rounding
    let row: (f64,) = sqlx::query_as(&format!(
        "SELECT amount::float8 FROM {} ORDER BY id DESC LIMIT 1",
        table
    ))
    .fetch_one(pool)
    .await?;
    assert!((row.0 - 67.90).abs() < 0.001);

    cleanup_table(pool, table).await?;
    Ok(())
}

// =============================================================================
// EDGE CASES - Composite Unique with NULL
// =============================================================================

#[tokio::test]
async fn test_composite_unique_with_partial_null() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_composite_partial_null";

    cleanup_table(pool, table).await?;

    sqlx::query(&format!(
        "CREATE TABLE {} (
            id SERIAL PRIMARY KEY,
            col_a TEXT,
            col_b TEXT,
            col_c TEXT NOT NULL,
            UNIQUE (col_a, col_b)
        )",
        table
    ))
    .execute(pool)
    .await?;

    // Insert with both non-null
    sqlx::query(&format!(
        "INSERT INTO {} (col_a, col_b, col_c) VALUES ($1, $2, $3)",
        table
    ))
    .bind("A1")
    .bind("B1")
    .bind("C1")
    .execute(pool)
    .await?;

    // Duplicate should fail
    let result = sqlx::query(&format!(
        "INSERT INTO {} (col_a, col_b, col_c) VALUES ($1, $2, $3)",
        table
    ))
    .bind("A1")
    .bind("B1")
    .bind("C2")
    .execute(pool)
    .await;
    assert!(result.is_err());

    // Insert with NULL in col_a
    sqlx::query(&format!(
        "INSERT INTO {} (col_a, col_b, col_c) VALUES ($1, $2, $3)",
        table
    ))
    .bind(Option::<String>::None)
    .bind("B1")
    .bind("C3")
    .execute(pool)
    .await?;

    // Another NULL + same col_b should work (NULL != NULL)
    sqlx::query(&format!(
        "INSERT INTO {} (col_a, col_b, col_c) VALUES ($1, $2, $3)",
        table
    ))
    .bind(Option::<String>::None)
    .bind("B1")
    .bind("C4")
    .execute(pool)
    .await?;

    // Both NULL should also work
    sqlx::query(&format!(
        "INSERT INTO {} (col_a, col_b, col_c) VALUES ($1, $2, $3)",
        table
    ))
    .bind(Option::<String>::None)
    .bind(Option::<String>::None)
    .bind("C5")
    .execute(pool)
    .await?;

    // Another both NULL should work too
    sqlx::query(&format!(
        "INSERT INTO {} (col_a, col_b, col_c) VALUES ($1, $2, $3)",
        table
    ))
    .bind(Option::<String>::None)
    .bind(Option::<String>::None)
    .bind("C6")
    .execute(pool)
    .await?;

    let count: (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM {}", table))
        .fetch_one(pool)
        .await?;
    assert_eq!(count.0, 5);

    cleanup_table(pool, table).await?;
    Ok(())
}

// =============================================================================
// EDGE CASES - Multiple Constraint Violations
// =============================================================================

#[tokio::test]
async fn test_multiple_constraint_violations() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_multi_constraint";

    cleanup_table(pool, table).await?;

    sqlx::query(&format!(
        "CREATE TABLE {} (
            id SERIAL PRIMARY KEY,
            email TEXT NOT NULL UNIQUE,
            age INTEGER NOT NULL CHECK (age >= 0)
        )",
        table
    ))
    .execute(pool)
    .await?;

    // Insert valid record
    sqlx::query(&format!(
        "INSERT INTO {} (email, age) VALUES ($1, $2)",
        table
    ))
    .bind("test@example.com")
    .bind(25)
    .execute(pool)
    .await?;

    // Violate both UNIQUE and CHECK - which error surfaces?
    let result = sqlx::query(&format!(
        "INSERT INTO {} (email, age) VALUES ($1, $2)",
        table
    ))
    .bind("test@example.com") // Duplicate
    .bind(-5) // Negative age
    .execute(pool)
    .await;

    // Should fail with one of the errors
    assert!(result.is_err());
    let err_str = result.unwrap_err().to_string().to_lowercase();
    // PostgreSQL will report one constraint violation (order depends on evaluation)
    assert!(
        err_str.contains("unique")
            || err_str.contains("duplicate")
            || err_str.contains("check")
            || err_str.contains("violates"),
        "Expected constraint error, got: {}",
        err_str
    );

    cleanup_table(pool, table).await?;
    Ok(())
}

// =============================================================================
// EDGE CASES - Deferred Constraints (if supported)
// =============================================================================

#[tokio::test]
async fn test_deferred_constraint() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_deferred";

    cleanup_table(pool, table).await?;

    // Create table with deferrable unique constraint
    sqlx::query(&format!(
        "CREATE TABLE {} (
            id SERIAL PRIMARY KEY,
            code TEXT NOT NULL,
            CONSTRAINT {}_code_unique UNIQUE (code) DEFERRABLE INITIALLY DEFERRED
        )",
        table, table
    ))
    .execute(pool)
    .await?;

    // Start transaction
    let mut tx = pool.begin().await?;

    // Insert first record
    sqlx::query(&format!("INSERT INTO {} (code) VALUES ($1)", table))
        .bind("CODE1")
        .execute(&mut *tx)
        .await?;

    // Insert duplicate - should NOT fail immediately (deferred)
    sqlx::query(&format!("INSERT INTO {} (code) VALUES ($1)", table))
        .bind("CODE1")
        .execute(&mut *tx)
        .await?;

    // Update second to be different before commit
    sqlx::query(&format!(
        "UPDATE {} SET code = $1 WHERE id = (SELECT MAX(id) FROM {})",
        table, table
    ))
    .bind("CODE2")
    .execute(&mut *tx)
    .await?;

    // Commit should succeed now
    tx.commit().await?;

    // Verify both records exist
    let count: (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM {}", table))
        .fetch_one(pool)
        .await?;
    assert_eq!(count.0, 2);

    cleanup_table(pool, table).await?;
    Ok(())
}

#[tokio::test]
async fn test_deferred_constraint_commit_failure() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_deferred_fail";

    cleanup_table(pool, table).await?;

    // Create table with deferrable unique constraint
    sqlx::query(&format!(
        "CREATE TABLE {} (
            id SERIAL PRIMARY KEY,
            code TEXT NOT NULL,
            CONSTRAINT {}_code_unique UNIQUE (code) DEFERRABLE INITIALLY DEFERRED
        )",
        table, table
    ))
    .execute(pool)
    .await?;

    // Start transaction
    let mut tx = pool.begin().await?;

    // Insert duplicates
    sqlx::query(&format!("INSERT INTO {} (code) VALUES ($1)", table))
        .bind("CODE1")
        .execute(&mut *tx)
        .await?;

    sqlx::query(&format!("INSERT INTO {} (code) VALUES ($1)", table))
        .bind("CODE1") // Duplicate, but deferred
        .execute(&mut *tx)
        .await?;

    // Commit should fail - constraint checked at commit
    let result = tx.commit().await;
    assert!(result.is_err());

    // Verify no records exist (rolled back)
    let count: (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM {}", table))
        .fetch_one(pool)
        .await?;
    assert_eq!(count.0, 0);

    cleanup_table(pool, table).await?;
    Ok(())
}

// =============================================================================
// EDGE CASES - Concurrent Constraint Violations
// =============================================================================

#[tokio::test]
async fn test_concurrent_unique_violation() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_concurrent_unique";

    cleanup_table(pool, table).await?;

    sqlx::query(&format!(
        "CREATE TABLE {} (
            id SERIAL PRIMARY KEY,
            code TEXT NOT NULL UNIQUE
        )",
        table
    ))
    .execute(pool)
    .await?;

    // Concurrent inserts of same value
    let mut handles = vec![];
    for _ in 0..5 {
        let pool_clone = pool.clone();
        let table_name = table.to_string();
        handles.push(tokio::spawn(async move {
            sqlx::query(&format!("INSERT INTO {} (code) VALUES ($1)", table_name))
                .bind("CONCURRENT_CODE")
                .execute(&pool_clone)
                .await
        }));
    }

    // Count successes and failures
    let mut successes = 0;
    let mut failures = 0;
    for handle in handles {
        match handle.await? {
            Ok(_) => successes += 1,
            Err(_) => failures += 1,
        }
    }

    // Exactly one should succeed
    assert_eq!(successes, 1, "Exactly one insert should succeed");
    assert_eq!(failures, 4, "Four inserts should fail");

    // Verify only one record exists
    let count: (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM {}", table))
        .fetch_one(pool)
        .await?;
    assert_eq!(count.0, 1);

    cleanup_table(pool, table).await?;
    Ok(())
}

// =============================================================================
// ERROR CASES - Invalid Table/Column Names
// =============================================================================

#[tokio::test]
async fn test_invalid_table_name() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();

    // Query non-existent table
    let result: Result<(i32,), _> = sqlx::query_as("SELECT 1 FROM nonexistent_table_xyz_123")
        .fetch_one(pool)
        .await;

    assert!(result.is_err());
    let err_str = result.unwrap_err().to_string().to_lowercase();
    assert!(
        err_str.contains("does not exist") || err_str.contains("relation"),
        "Expected table not found error, got: {}",
        err_str
    );

    Ok(())
}

#[tokio::test]
async fn test_invalid_column_name() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_invalid_col";

    cleanup_table(pool, table).await?;

    sqlx::query(&format!(
        "CREATE TABLE {} (id SERIAL PRIMARY KEY, name TEXT NOT NULL)",
        table
    ))
    .execute(pool)
    .await?;

    // Query non-existent column
    let result: Result<(i32,), _> =
        sqlx::query_as(&format!("SELECT nonexistent_column FROM {}", table))
            .fetch_one(pool)
            .await;

    assert!(result.is_err());
    let err_str = result.unwrap_err().to_string().to_lowercase();
    assert!(
        err_str.contains("does not exist") || err_str.contains("column"),
        "Expected column not found error, got: {}",
        err_str
    );

    cleanup_table(pool, table).await?;
    Ok(())
}
