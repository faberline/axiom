//! Integration tests for Transaction operations.
//!
//! These tests require a PostgreSQL database to be running.
//! Set DATABASE_URL environment variable to customize connection.
//! Default: postgresql://localhost/test_db
//!
//! Run with: cargo test -p cclab-titan --test test_transaction

use pg::{Connection, IsolationLevel, PoolConfig, Transaction, TransactionOptions};
use qc::{expect, AssertionError};

/// Helper to get database URL from environment
fn get_database_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql://localhost/test_db".to_string())
}

/// Helper to create a test table for transaction tests
async fn setup_test_table(pool: &sqlx::PgPool, table_name: &str) -> Result<(), sqlx::Error> {
    sqlx::query(&format!("DROP TABLE IF EXISTS {} CASCADE", table_name))
        .execute(pool)
        .await?;

    sqlx::query(&format!(
        "CREATE TABLE {} (
            id BIGSERIAL PRIMARY KEY,
            name TEXT NOT NULL,
            value INTEGER NOT NULL
        )",
        table_name
    ))
    .execute(pool)
    .await?;

    Ok(())
}

/// Helper to cleanup test table
async fn cleanup_test_table(pool: &sqlx::PgPool, table_name: &str) -> Result<(), sqlx::Error> {
    sqlx::query(&format!("DROP TABLE IF EXISTS {} CASCADE", table_name))
        .execute(pool)
        .await?;
    Ok(())
}

/// Helper to count rows in table
async fn count_rows(pool: &sqlx::PgPool, table_name: &str) -> Result<i64, sqlx::Error> {
    let row: (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM {}", table_name))
        .fetch_one(pool)
        .await?;
    Ok(row.0)
}

/// Helper to check if a record exists by name
async fn record_exists(
    pool: &sqlx::PgPool,
    table_name: &str,
    name: &str,
) -> Result<bool, sqlx::Error> {
    let row: (i64,) = sqlx::query_as(&format!(
        "SELECT COUNT(*) FROM {} WHERE name = $1",
        table_name
    ))
    .bind(name)
    .fetch_one(pool)
    .await?;
    Ok(row.0 > 0)
}

// =============================================================================
// Commit/Rollback Tests
// =============================================================================

#[tokio::test]
async fn test_transaction_commit() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_txn_commit";

    setup_test_table(pool, table).await?;

    // Begin transaction, insert record, commit
    let mut txn = Transaction::begin(&conn, IsolationLevel::ReadCommitted).await?;

    sqlx::query(&format!(
        "INSERT INTO {} (name, value) VALUES ($1, $2)",
        table
    ))
    .bind("Alice")
    .bind(100)
    .execute(txn.as_mut_transaction().as_mut())
    .await?;

    txn.commit().await?;

    // Verify record exists with a new connection
    let exists = record_exists(pool, table, "Alice").await?;
    expect(exists).to_be_true()?;

    cleanup_test_table(pool, table).await?;
    Ok(())
}

#[tokio::test]
async fn test_transaction_rollback() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_txn_rollback";

    setup_test_table(pool, table).await?;

    // Begin transaction, insert record, rollback
    let mut txn = Transaction::begin(&conn, IsolationLevel::ReadCommitted).await?;

    sqlx::query(&format!(
        "INSERT INTO {} (name, value) VALUES ($1, $2)",
        table
    ))
    .bind("Bob")
    .bind(200)
    .execute(txn.as_mut_transaction().as_mut())
    .await?;

    txn.rollback().await?;

    // Verify record does NOT exist
    let exists = record_exists(pool, table, "Bob").await?;
    expect(exists).to_be_false()?;

    cleanup_test_table(pool, table).await?;
    Ok(())
}

// =============================================================================
// Savepoint Tests
// =============================================================================

#[tokio::test]
async fn test_savepoint_partial_rollback() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_txn_savepoint";

    setup_test_table(pool, table).await?;

    // Begin transaction
    let mut txn = Transaction::begin(&conn, IsolationLevel::ReadCommitted).await?;

    // Insert A
    sqlx::query(&format!(
        "INSERT INTO {} (name, value) VALUES ($1, $2)",
        table
    ))
    .bind("RecordA")
    .bind(100)
    .execute(txn.as_mut_transaction().as_mut())
    .await?;

    // Create savepoint
    txn.savepoint("sp1").await?;

    // Insert B after savepoint
    sqlx::query(&format!(
        "INSERT INTO {} (name, value) VALUES ($1, $2)",
        table
    ))
    .bind("RecordB")
    .bind(200)
    .execute(txn.as_mut_transaction().as_mut())
    .await?;

    // Rollback to savepoint (undoes B, keeps A)
    txn.rollback_to("sp1").await?;

    // Commit transaction
    txn.commit().await?;

    // Verify: A exists, B does not
    let a_exists = record_exists(pool, table, "RecordA").await?;
    let b_exists = record_exists(pool, table, "RecordB").await?;

    expect(a_exists).to_be_true()?;
    expect(b_exists).to_be_false()?;

    cleanup_test_table(pool, table).await?;
    Ok(())
}

#[tokio::test]
async fn test_release_savepoint() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_txn_release_sp";

    setup_test_table(pool, table).await?;

    // Begin transaction
    let mut txn = Transaction::begin(&conn, IsolationLevel::ReadCommitted).await?;

    // Insert A
    sqlx::query(&format!(
        "INSERT INTO {} (name, value) VALUES ($1, $2)",
        table
    ))
    .bind("RecordA")
    .bind(100)
    .execute(txn.as_mut_transaction().as_mut())
    .await?;

    // Create savepoint
    txn.savepoint("sp1").await?;

    // Insert B after savepoint
    sqlx::query(&format!(
        "INSERT INTO {} (name, value) VALUES ($1, $2)",
        table
    ))
    .bind("RecordB")
    .bind(200)
    .execute(txn.as_mut_transaction().as_mut())
    .await?;

    // Release savepoint (confirms changes since savepoint)
    txn.release_savepoint("sp1").await?;

    // Commit transaction
    txn.commit().await?;

    // Verify: both A and B exist
    let a_exists = record_exists(pool, table, "RecordA").await?;
    let b_exists = record_exists(pool, table, "RecordB").await?;

    expect(a_exists).to_be_true()?;
    expect(b_exists).to_be_true()?;

    cleanup_test_table(pool, table).await?;
    Ok(())
}

#[tokio::test]
async fn test_invalid_savepoint_name() -> Result<(), AssertionError> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await.unwrap();

    // Begin transaction
    let mut txn = Transaction::begin(&conn, IsolationLevel::ReadCommitted)
        .await
        .unwrap();

    // Invalid savepoint name with special characters should fail validation
    // Note: Using dash instead of space as dash is explicitly rejected
    let result = txn.savepoint("invalid-name").await;

    expect(result.is_err()).to_be_true()?;

    // Verify error mentions invalid character
    let err = result.unwrap_err();
    let err_str = err.to_string();
    expect(err_str.contains("invalid character") || err_str.contains("Invalid")).to_be_true()?;

    // Transaction will be auto-rolled back on drop
    Ok(())
}

// =============================================================================
// Isolation Level Tests
// =============================================================================

#[tokio::test]
async fn test_isolation_level_repeatable_read() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;

    // Begin transaction with RepeatableRead isolation
    let mut txn = Transaction::begin(&conn, IsolationLevel::RepeatableRead).await?;

    // Query current isolation level
    let row: (String,) = sqlx::query_as("SHOW transaction_isolation")
        .fetch_one(txn.as_mut_transaction().as_mut())
        .await?;

    expect(row.0.as_str()).to_equal(&"repeatable read")?;

    txn.rollback().await?;
    Ok(())
}

#[tokio::test]
async fn test_isolation_level_serializable() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;

    // Begin transaction with Serializable isolation
    let mut txn = Transaction::begin(&conn, IsolationLevel::Serializable).await?;

    // Query current isolation level
    let row: (String,) = sqlx::query_as("SHOW transaction_isolation")
        .fetch_one(txn.as_mut_transaction().as_mut())
        .await?;

    expect(row.0.as_str()).to_equal(&"serializable")?;

    txn.rollback().await?;
    Ok(())
}

#[tokio::test]
async fn test_isolation_level_read_committed() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;

    // Begin transaction with ReadCommitted isolation (default)
    let mut txn = Transaction::begin(&conn, IsolationLevel::ReadCommitted).await?;

    // Query current isolation level
    let row: (String,) = sqlx::query_as("SHOW transaction_isolation")
        .fetch_one(txn.as_mut_transaction().as_mut())
        .await?;

    expect(row.0.as_str()).to_equal(&"read committed")?;

    txn.rollback().await?;
    Ok(())
}

// =============================================================================
// Auto-Rollback on Drop Tests
// =============================================================================

#[tokio::test]
async fn test_auto_rollback_on_drop() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_txn_auto_rollback";

    setup_test_table(pool, table).await?;

    // Scope to ensure transaction is dropped
    {
        let mut txn = Transaction::begin(&conn, IsolationLevel::ReadCommitted).await?;

        // Insert record
        sqlx::query(&format!(
            "INSERT INTO {} (name, value) VALUES ($1, $2)",
            table
        ))
        .bind("Dropped")
        .bind(999)
        .execute(txn.as_mut_transaction().as_mut())
        .await?;

        // Transaction is dropped here without commit or explicit rollback
        // SQLx should auto-rollback
    }

    // Verify record does NOT exist (auto-rolled back)
    let exists = record_exists(pool, table, "Dropped").await?;
    expect(exists).to_be_false()?;

    cleanup_test_table(pool, table).await?;
    Ok(())
}

// =============================================================================
// Additional Edge Case Tests
// =============================================================================

#[tokio::test]
async fn test_multiple_savepoints() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_txn_multi_sp";

    setup_test_table(pool, table).await?;

    let mut txn = Transaction::begin(&conn, IsolationLevel::ReadCommitted).await?;

    // Insert A
    sqlx::query(&format!(
        "INSERT INTO {} (name, value) VALUES ($1, $2)",
        table
    ))
    .bind("A")
    .bind(1)
    .execute(txn.as_mut_transaction().as_mut())
    .await?;

    txn.savepoint("sp1").await?;

    // Insert B
    sqlx::query(&format!(
        "INSERT INTO {} (name, value) VALUES ($1, $2)",
        table
    ))
    .bind("B")
    .bind(2)
    .execute(txn.as_mut_transaction().as_mut())
    .await?;

    txn.savepoint("sp2").await?;

    // Insert C
    sqlx::query(&format!(
        "INSERT INTO {} (name, value) VALUES ($1, $2)",
        table
    ))
    .bind("C")
    .bind(3)
    .execute(txn.as_mut_transaction().as_mut())
    .await?;

    // Rollback to sp2 (undoes C only)
    txn.rollback_to("sp2").await?;

    // Rollback to sp1 (undoes B as well)
    txn.rollback_to("sp1").await?;

    txn.commit().await?;

    // Verify: only A exists
    let a_exists = record_exists(pool, table, "A").await?;
    let b_exists = record_exists(pool, table, "B").await?;
    let c_exists = record_exists(pool, table, "C").await?;

    expect(a_exists).to_be_true()?;
    expect(b_exists).to_be_false()?;
    expect(c_exists).to_be_false()?;

    let count = count_rows(pool, table).await?;
    expect(count).to_equal(&1)?;

    cleanup_test_table(pool, table).await?;
    Ok(())
}

// =============================================================================
// Additional Isolation Level Tests
// =============================================================================

#[tokio::test]
async fn test_isolation_level_read_uncommitted() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;

    // Begin transaction with ReadUncommitted isolation
    // Note: PostgreSQL treats READ UNCOMMITTED as READ COMMITTED
    let mut txn = Transaction::begin(&conn, IsolationLevel::ReadUncommitted).await?;

    // Query current isolation level
    let row: (String,) = sqlx::query_as("SHOW transaction_isolation")
        .fetch_one(txn.as_mut_transaction().as_mut())
        .await?;

    // PostgreSQL maps READ UNCOMMITTED to READ COMMITTED
    expect(row.0.as_str()).to_equal(&"read uncommitted")?;

    txn.rollback().await?;
    Ok(())
}

// =============================================================================
// Transaction Options Tests
// =============================================================================

#[tokio::test]
async fn test_transaction_read_only_mode() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_txn_read_only";

    setup_test_table(pool, table).await?;

    // Begin read-only transaction
    let options = TransactionOptions::new()
        .isolation_level(IsolationLevel::ReadCommitted)
        .read_only();
    let mut txn = Transaction::begin_with_options(&conn, options).await?;

    // Attempting to INSERT in read-only transaction should fail
    let result = sqlx::query(&format!(
        "INSERT INTO {} (name, value) VALUES ($1, $2)",
        table
    ))
    .bind("ReadOnlyTest")
    .bind(100)
    .execute(txn.as_mut_transaction().as_mut())
    .await;

    // Should fail with error about read-only transaction
    expect(result.is_err()).to_be_true()?;
    let err = result.unwrap_err();
    let err_str = err.to_string().to_lowercase();
    expect(err_str.contains("read-only") || err_str.contains("read only")).to_be_true()?;

    // Transaction will be auto-rolled back on drop
    drop(txn);

    cleanup_test_table(pool, table).await?;
    Ok(())
}

#[tokio::test]
async fn test_transaction_read_write_mode() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_txn_read_write";

    setup_test_table(pool, table).await?;

    // Begin read-write transaction (explicit)
    let options = TransactionOptions::new()
        .isolation_level(IsolationLevel::ReadCommitted)
        .read_write();
    let mut txn = Transaction::begin_with_options(&conn, options).await?;

    // INSERT should succeed in read-write transaction
    sqlx::query(&format!(
        "INSERT INTO {} (name, value) VALUES ($1, $2)",
        table
    ))
    .bind("ReadWriteTest")
    .bind(200)
    .execute(txn.as_mut_transaction().as_mut())
    .await?;

    txn.commit().await?;

    // Verify record exists
    let exists = record_exists(pool, table, "ReadWriteTest").await?;
    expect(exists).to_be_true()?;

    cleanup_test_table(pool, table).await?;
    Ok(())
}

#[tokio::test]
async fn test_transaction_deferrable() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;

    // DEFERRABLE is only meaningful for SERIALIZABLE READ ONLY transactions
    let options = TransactionOptions::new()
        .isolation_level(IsolationLevel::Serializable)
        .read_only()
        .deferrable(true);

    let mut txn = Transaction::begin_with_options(&conn, options).await?;

    // Verify transaction was created successfully
    // Query to verify we can execute within the transaction
    let row: (String,) = sqlx::query_as("SHOW transaction_isolation")
        .fetch_one(txn.as_mut_transaction().as_mut())
        .await?;

    expect(row.0.as_str()).to_equal(&"serializable")?;

    txn.rollback().await?;
    Ok(())
}

// =============================================================================
// Nested Transaction Simulation Tests
// =============================================================================

#[tokio::test]
async fn test_nested_savepoint_simulation() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_txn_nested";

    setup_test_table(pool, table).await?;

    // Simulate nested transactions using savepoints
    let mut txn = Transaction::begin(&conn, IsolationLevel::ReadCommitted).await?;

    // "Outer transaction" - insert parent record
    sqlx::query(&format!(
        "INSERT INTO {} (name, value) VALUES ($1, $2)",
        table
    ))
    .bind("Parent")
    .bind(1)
    .execute(txn.as_mut_transaction().as_mut())
    .await?;

    // "Inner transaction 1" via savepoint
    txn.savepoint("inner1").await?;
    sqlx::query(&format!(
        "INSERT INTO {} (name, value) VALUES ($1, $2)",
        table
    ))
    .bind("Child1")
    .bind(2)
    .execute(txn.as_mut_transaction().as_mut())
    .await?;

    // "Inner transaction 2" (nested within inner1) via savepoint
    txn.savepoint("inner2").await?;
    sqlx::query(&format!(
        "INSERT INTO {} (name, value) VALUES ($1, $2)",
        table
    ))
    .bind("Child2")
    .bind(3)
    .execute(txn.as_mut_transaction().as_mut())
    .await?;

    // Rollback inner2 only (keeps Parent and Child1)
    txn.rollback_to("inner2").await?;

    // Release inner1 (commits Child1 within the outer transaction context)
    txn.release_savepoint("inner1").await?;

    // Commit outer transaction
    txn.commit().await?;

    // Verify: Parent and Child1 exist, Child2 does not
    let parent_exists = record_exists(pool, table, "Parent").await?;
    let child1_exists = record_exists(pool, table, "Child1").await?;
    let child2_exists = record_exists(pool, table, "Child2").await?;

    expect(parent_exists).to_be_true()?;
    expect(child1_exists).to_be_true()?;
    expect(child2_exists).to_be_false()?;

    let count = count_rows(pool, table).await?;
    expect(count).to_equal(&2)?;

    cleanup_test_table(pool, table).await?;
    Ok(())
}
