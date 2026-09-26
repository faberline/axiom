//! Integration tests for database migration system.
//!
//! These tests verify MigrationRunner operations including apply, revert,
//! checksum validation, status tracking, and rollback consistency.
//!
//! These tests require a PostgreSQL database to be running.
//! Set POSTGRES_URL environment variable or skip with SKIP_INTEGRATION=true
//!
//! Run tests with: cargo test --package cclab-titan --test test_migration -- --ignored

use pg::{Connection, Migration, MigrationRunner, PoolConfig};
use qc::{expect, AssertionError};
use std::fs;
use tempfile::TempDir;

/// Helper to create a test database connection
async fn create_test_connection() -> Connection {
    let uri = std::env::var("POSTGRES_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .unwrap_or_else(|_| "postgresql://localhost/test_db".to_string());

    Connection::new(&uri, PoolConfig::default())
        .await
        .expect("Failed to connect to database")
}

/// Helper to create a migration runner and initialize it
/// Returns the runner and a connection for cleanup operations
async fn create_migration_runner(table_name: &str) -> (MigrationRunner, Connection) {
    let conn = create_test_connection().await;
    let runner = MigrationRunner::new(conn.clone(), Some(table_name.to_string()));
    runner
        .init()
        .await
        .expect("Failed to initialize migration runner");
    (runner, conn)
}

/// Helper to clean up migration table
async fn cleanup_migration_table(conn: &Connection, table_name: &str) {
    let pool = conn.pool();
    let sql = format!("DROP TABLE IF EXISTS {} CASCADE", table_name);
    sqlx::query(&sql)
        .execute(pool)
        .await
        .expect("Failed to drop migration table");
}

/// Helper to check if a table exists
async fn table_exists(conn: &Connection, table_name: &str) -> bool {
    let pool = conn.pool();
    let result: Option<String> = sqlx::query_scalar(
        "SELECT table_name FROM information_schema.tables
         WHERE table_schema = 'public' AND table_name = $1",
    )
    .bind(table_name)
    .fetch_optional(pool)
    .await
    .expect("Failed to check table existence");

    result.is_some()
}

#[tokio::test]
#[ignore]
async fn test_migration_runner_init() -> Result<(), AssertionError> {
    let table_name = "_test_migrations_init";
    let (_runner, conn) = create_migration_runner(table_name).await;

    // Verify migration table was created
    expect(table_exists(&conn, table_name).await).to_be_true()?;

    // Verify it has the correct columns
    let pool = conn.pool();
    let columns: Vec<String> = sqlx::query_scalar(
        "SELECT column_name FROM information_schema.columns
         WHERE table_name = $1
         ORDER BY ordinal_position",
    )
    .bind(table_name)
    .fetch_all(pool)
    .await
    .unwrap();

    expect(columns.contains(&"version".to_string())).to_be_true()?;
    expect(columns.contains(&"description".to_string())).to_be_true()?;
    expect(columns.contains(&"applied_at".to_string())).to_be_true()?;
    expect(columns.contains(&"checksum".to_string())).to_be_true()?;

    // Clean up
    cleanup_migration_table(&conn, table_name).await;

    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_apply_migration_creates_table() -> Result<(), AssertionError> {
    let migration_table = "_test_migrations_apply";
    let (runner, conn) = create_migration_runner(migration_table).await;

    // Create a simple migration
    let migration = Migration::new(
        "20250112_000001".to_string(),
        "create test users table".to_string(),
        "CREATE TABLE test_migration_users (
            id SERIAL PRIMARY KEY,
            name TEXT NOT NULL,
            email TEXT UNIQUE NOT NULL
        )"
        .to_string(),
        "DROP TABLE test_migration_users".to_string(),
    );

    // Apply migration
    runner.apply(&migration).await.unwrap();

    // Verify table was created
    expect(table_exists(&conn, "test_migration_users").await).to_be_true()?;

    // Verify migration was recorded
    let applied = runner.applied_migrations().await.unwrap();
    expect(applied.contains(&"20250112_000001".to_string())).to_be_true()?;

    // Clean up
    let pool = conn.pool();
    sqlx::query("DROP TABLE IF EXISTS test_migration_users CASCADE")
        .execute(pool)
        .await
        .unwrap();
    cleanup_migration_table(&conn, migration_table).await;

    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_revert_migration_drops_table() -> Result<(), AssertionError> {
    let migration_table = "_test_migrations_revert";
    let (runner, conn) = create_migration_runner(migration_table).await;

    // Create and apply migration
    let migration = Migration::new(
        "20250112_000002".to_string(),
        "create test products table".to_string(),
        "CREATE TABLE test_migration_products (
            id SERIAL PRIMARY KEY,
            name TEXT NOT NULL,
            price DECIMAL(10, 2)
        )"
        .to_string(),
        "DROP TABLE test_migration_products".to_string(),
    );

    runner.apply(&migration).await.unwrap();
    expect(table_exists(&conn, "test_migration_products").await).to_be_true()?;

    // Revert migration
    runner.revert(&migration).await.unwrap();

    // Verify table was dropped
    expect(table_exists(&conn, "test_migration_products").await).to_be_false()?;

    // Verify migration record was removed
    let applied = runner.applied_migrations().await.unwrap();
    expect(applied.contains(&"20250112_000002".to_string())).to_be_false()?;

    // Clean up
    cleanup_migration_table(&conn, migration_table).await;

    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_checksum_validation_detects_modification() -> Result<(), AssertionError> {
    let migration_table = "_test_migrations_checksum";
    let (runner, conn) = create_migration_runner(migration_table).await;

    // Create and apply original migration
    let original_migration = Migration::new(
        "20250112_000003".to_string(),
        "create test orders table".to_string(),
        "CREATE TABLE test_migration_orders (
            id SERIAL PRIMARY KEY,
            total DECIMAL(10, 2)
        )"
        .to_string(),
        "DROP TABLE test_migration_orders".to_string(),
    );

    runner.apply(&original_migration).await.unwrap();

    // Create a modified migration with same version but different content
    let modified_migration = Migration::new(
        "20250112_000003".to_string(),
        "create test orders table MODIFIED".to_string(),
        "CREATE TABLE test_migration_orders (
            id SERIAL PRIMARY KEY,
            total DECIMAL(10, 2),
            status TEXT -- MODIFIED: added new column
        )"
        .to_string(),
        "DROP TABLE test_migration_orders".to_string(),
    );

    // Attempting to apply modified migration should fail due to checksum mismatch
    let result = runner.apply(&modified_migration).await;
    expect(result.is_err()).to_be_true()?;

    // Verify error message mentions checksum
    let err_msg = format!("{:?}", result.unwrap_err());
    expect(err_msg.contains("Checksum mismatch")).to_be_true()?;

    // Clean up
    let pool = conn.pool();
    sqlx::query("DROP TABLE IF EXISTS test_migration_orders CASCADE")
        .execute(pool)
        .await
        .unwrap();
    cleanup_migration_table(&conn, migration_table).await;

    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_migration_status_tracking() -> Result<(), AssertionError> {
    let migration_table = "_test_migrations_status";
    let (runner, conn) = create_migration_runner(migration_table).await;

    // Create multiple migrations
    let migrations = vec![
        Migration::new(
            "20250112_000004".to_string(),
            "create categories".to_string(),
            "CREATE TABLE test_migration_categories (id SERIAL PRIMARY KEY, name TEXT)".to_string(),
            "DROP TABLE test_migration_categories".to_string(),
        ),
        Migration::new(
            "20250112_000005".to_string(),
            "create tags".to_string(),
            "CREATE TABLE test_migration_tags (id SERIAL PRIMARY KEY, label TEXT)".to_string(),
            "DROP TABLE test_migration_tags".to_string(),
        ),
        Migration::new(
            "20250112_000006".to_string(),
            "create comments".to_string(),
            "CREATE TABLE test_migration_comments (id SERIAL PRIMARY KEY, content TEXT)"
                .to_string(),
            "DROP TABLE test_migration_comments".to_string(),
        ),
    ];

    // Apply first two migrations
    runner.apply(&migrations[0]).await.unwrap();
    runner.apply(&migrations[1]).await.unwrap();

    // Check status
    let status = runner.status(&migrations).await.unwrap();

    expect(status.applied.len()).to_equal(&2)?;
    expect(status.applied.contains(&"20250112_000004".to_string())).to_be_true()?;
    expect(status.applied.contains(&"20250112_000005".to_string())).to_be_true()?;

    expect(status.pending.len()).to_equal(&1)?;
    expect(status.pending.contains(&"20250112_000006".to_string())).to_be_true()?;

    // Verify applied migrations have details
    let applied_details = runner.applied_migrations_with_details().await.unwrap();
    expect(applied_details.len()).to_equal(&2)?;

    // Verify first migration details
    let first = &applied_details[0];
    expect(first.version.as_str()).to_equal(&"20250112_000004")?;
    expect(first.name.as_str()).to_equal(&"create categories")?;
    expect(first.applied_at.is_some()).to_be_true()?;
    expect(!first.checksum.is_empty()).to_be_true()?;

    // Clean up
    let pool = conn.pool();
    sqlx::query("DROP TABLE IF EXISTS test_migration_categories CASCADE")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DROP TABLE IF EXISTS test_migration_tags CASCADE")
        .execute(pool)
        .await
        .unwrap();
    cleanup_migration_table(&conn, migration_table).await;

    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_rollback_consistency() -> Result<(), AssertionError> {
    let migration_table = "_test_migrations_rollback";
    let (runner, conn) = create_migration_runner(migration_table).await;

    // Create a series of migrations
    let migrations = vec![
        Migration::new(
            "20250112_000007".to_string(),
            "create authors".to_string(),
            "CREATE TABLE test_migration_authors (id SERIAL PRIMARY KEY, name TEXT)".to_string(),
            "DROP TABLE test_migration_authors".to_string(),
        ),
        Migration::new(
            "20250112_000008".to_string(),
            "create books".to_string(),
            "CREATE TABLE test_migration_books (
                id SERIAL PRIMARY KEY,
                title TEXT,
                author_id INTEGER REFERENCES test_migration_authors(id)
            )"
            .to_string(),
            "DROP TABLE test_migration_books".to_string(),
        ),
        Migration::new(
            "20250112_000009".to_string(),
            "create reviews".to_string(),
            "CREATE TABLE test_migration_reviews (
                id SERIAL PRIMARY KEY,
                book_id INTEGER REFERENCES test_migration_books(id),
                rating INTEGER
            )"
            .to_string(),
            "DROP TABLE test_migration_reviews".to_string(),
        ),
    ];

    // Apply all migrations
    for migration in &migrations {
        runner.apply(migration).await.unwrap();
    }

    // Verify all tables exist
    expect(table_exists(&conn, "test_migration_authors").await).to_be_true()?;
    expect(table_exists(&conn, "test_migration_books").await).to_be_true()?;
    expect(table_exists(&conn, "test_migration_reviews").await).to_be_true()?;

    // Rollback last 2 migrations
    let reverted = runner.rollback(&migrations, 2).await.unwrap();
    expect(reverted.len()).to_equal(&2)?;
    expect(reverted.contains(&"20250112_000009".to_string())).to_be_true()?;
    expect(reverted.contains(&"20250112_000008".to_string())).to_be_true()?;

    // Verify tables were dropped in correct order (reviews, then books)
    expect(table_exists(&conn, "test_migration_reviews").await).to_be_false()?;
    expect(table_exists(&conn, "test_migration_books").await).to_be_false()?;
    expect(table_exists(&conn, "test_migration_authors").await).to_be_true()?;

    // Verify migration records were removed
    let applied = runner.applied_migrations().await.unwrap();
    expect(applied.len()).to_equal(&1)?;
    expect(applied.contains(&"20250112_000007".to_string())).to_be_true()?;

    // Clean up
    let pool = conn.pool();
    sqlx::query("DROP TABLE IF EXISTS test_migration_authors CASCADE")
        .execute(pool)
        .await
        .unwrap();
    cleanup_migration_table(&conn, migration_table).await;

    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_migrate_applies_all_pending() -> Result<(), AssertionError> {
    let migration_table = "_test_migrations_migrate";
    let (runner, conn) = create_migration_runner(migration_table).await;

    // Create multiple migrations
    let migrations = vec![
        Migration::new(
            "20250112_000010".to_string(),
            "create sessions".to_string(),
            "CREATE TABLE test_migration_sessions (id SERIAL PRIMARY KEY, token TEXT)".to_string(),
            "DROP TABLE test_migration_sessions".to_string(),
        ),
        Migration::new(
            "20250112_000011".to_string(),
            "create logs".to_string(),
            "CREATE TABLE test_migration_logs (id SERIAL PRIMARY KEY, message TEXT)".to_string(),
            "DROP TABLE test_migration_logs".to_string(),
        ),
    ];

    // Apply all pending migrations at once
    let applied = runner.migrate(&migrations).await.unwrap();

    expect(applied.len()).to_equal(&2)?;
    expect(applied.contains(&"20250112_000010".to_string())).to_be_true()?;
    expect(applied.contains(&"20250112_000011".to_string())).to_be_true()?;

    // Verify tables were created
    expect(table_exists(&conn, "test_migration_sessions").await).to_be_true()?;
    expect(table_exists(&conn, "test_migration_logs").await).to_be_true()?;

    // Running migrate again should apply nothing
    let applied_again = runner.migrate(&migrations).await.unwrap();
    expect(applied_again.len()).to_equal(&0)?;

    // Clean up
    let pool = conn.pool();
    sqlx::query("DROP TABLE IF EXISTS test_migration_sessions CASCADE")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DROP TABLE IF EXISTS test_migration_logs CASCADE")
        .execute(pool)
        .await
        .unwrap();
    cleanup_migration_table(&conn, migration_table).await;

    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_migration_with_multiple_statements() -> Result<(), AssertionError> {
    let migration_table = "_test_migrations_multi_stmt";
    let (runner, conn) = create_migration_runner(migration_table).await;

    // Create migration with multiple SQL statements
    let migration = Migration::new(
        "20250112_000012".to_string(),
        "create complex schema".to_string(),
        r#"
        CREATE TABLE test_migration_customers (
            id SERIAL PRIMARY KEY,
            name TEXT NOT NULL,
            email TEXT UNIQUE NOT NULL
        );

        CREATE INDEX idx_customers_email ON test_migration_customers(email);

        CREATE TABLE test_migration_invoices (
            id SERIAL PRIMARY KEY,
            customer_id INTEGER REFERENCES test_migration_customers(id),
            amount DECIMAL(10, 2) NOT NULL,
            created_at TIMESTAMPTZ DEFAULT NOW()
        );

        CREATE INDEX idx_invoices_customer ON test_migration_invoices(customer_id);
        "#
        .to_string(),
        r#"
        DROP TABLE test_migration_invoices;
        DROP TABLE test_migration_customers;
        "#
        .to_string(),
    );

    // Apply migration
    runner.apply(&migration).await.unwrap();

    // Verify both tables were created
    expect(table_exists(&conn, "test_migration_customers").await).to_be_true()?;
    expect(table_exists(&conn, "test_migration_invoices").await).to_be_true()?;

    // Verify indexes were created
    let pool = conn.pool();
    let index_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pg_indexes
         WHERE schemaname = 'public'
         AND tablename IN ('test_migration_customers', 'test_migration_invoices')",
    )
    .fetch_one(pool)
    .await
    .unwrap();

    expect(index_count).to_be_greater_than(&1)?;

    // Revert migration
    runner.revert(&migration).await.unwrap();

    // Verify tables were dropped
    expect(table_exists(&conn, "test_migration_customers").await).to_be_false()?;
    expect(table_exists(&conn, "test_migration_invoices").await).to_be_false()?;

    // Clean up
    cleanup_migration_table(&conn, migration_table).await;

    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_migration_from_file() -> Result<(), AssertionError> {
    // Create a temporary directory for migration files
    let temp_dir = TempDir::new().unwrap();
    let migration_file = temp_dir
        .path()
        .join("20250112_120000_create_test_table.sql");

    // Write a migration file
    let migration_content = r#"
-- Description: Create test table for file loading
-- Migration: 20250112_120000_create_test_table

-- UP
CREATE TABLE test_migration_from_file (
    id SERIAL PRIMARY KEY,
    data TEXT NOT NULL
);

-- DOWN
DROP TABLE test_migration_from_file;
"#;

    fs::write(&migration_file, migration_content).unwrap();

    // Load migration from file
    let migration = Migration::from_file(&migration_file).unwrap();

    // Verify migration was parsed correctly
    expect(migration.version.as_str()).to_equal(&"20250112_120000")?;
    expect(
        migration
            .name
            .contains("Create test table for file loading"),
    )
    .to_be_true()?;
    expect(
        migration
            .up
            .contains("CREATE TABLE test_migration_from_file"),
    )
    .to_be_true()?;
    expect(
        migration
            .down
            .contains("DROP TABLE test_migration_from_file"),
    )
    .to_be_true()?;
    expect(!migration.checksum.is_empty()).to_be_true()?;

    // Apply the migration
    let migration_table = "_test_migrations_from_file";
    let (runner, conn) = create_migration_runner(migration_table).await;
    runner.apply(&migration).await.unwrap();

    // Verify table was created
    expect(table_exists(&conn, "test_migration_from_file").await).to_be_true()?;

    // Clean up
    let pool = conn.pool();
    sqlx::query("DROP TABLE IF EXISTS test_migration_from_file CASCADE")
        .execute(pool)
        .await
        .unwrap();
    cleanup_migration_table(&conn, migration_table).await;

    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_load_from_directory() -> Result<(), AssertionError> {
    // Create a temporary directory with multiple migration files
    let temp_dir = TempDir::new().unwrap();

    // Create migration files
    let files = vec![
        (
            "20250112_000001_first.sql",
            "-- UP\nCREATE TABLE first (id SERIAL);\n-- DOWN\nDROP TABLE first;",
        ),
        (
            "20250112_000002_second.sql",
            "-- UP\nCREATE TABLE second (id SERIAL);\n-- DOWN\nDROP TABLE second;",
        ),
        (
            "20250112_000003_third.sql",
            "-- UP\nCREATE TABLE third (id SERIAL);\n-- DOWN\nDROP TABLE third;",
        ),
    ];

    for (filename, content) in &files {
        let file_path = temp_dir.path().join(filename);
        fs::write(file_path, content).unwrap();
    }

    // Load all migrations from directory
    let migrations = MigrationRunner::load_from_directory(temp_dir.path()).unwrap();

    // Verify all migrations were loaded
    expect(migrations.len()).to_equal(&3)?;

    // Verify they are sorted by version
    expect(migrations[0].version.as_str()).to_equal(&"20250112_000001")?;
    expect(migrations[1].version.as_str()).to_equal(&"20250112_000002")?;
    expect(migrations[2].version.as_str()).to_equal(&"20250112_000003")?;

    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_migration_transaction_rollback_on_error() -> Result<(), AssertionError> {
    let migration_table = "_test_migrations_tx_rollback";
    let (runner, conn) = create_migration_runner(migration_table).await;

    // Create migration with invalid SQL in the middle
    let migration = Migration::new(
        "20250112_000013".to_string(),
        "failing migration".to_string(),
        r#"
        CREATE TABLE test_migration_temp1 (id SERIAL PRIMARY KEY);
        CREATE TABLE test_migration_temp2 (id SERIAL PRIMARY KEY);
        THIS IS INVALID SQL;
        CREATE TABLE test_migration_temp3 (id SERIAL PRIMARY KEY);
        "#
        .to_string(),
        r#"
        DROP TABLE test_migration_temp3;
        DROP TABLE test_migration_temp2;
        DROP TABLE test_migration_temp1;
        "#
        .to_string(),
    );

    // Attempt to apply migration (should fail)
    let result = runner.apply(&migration).await;
    expect(result.is_err()).to_be_true()?;

    // Verify no tables were created (transaction rolled back)
    expect(table_exists(&conn, "test_migration_temp1").await).to_be_false()?;
    expect(table_exists(&conn, "test_migration_temp2").await).to_be_false()?;
    expect(table_exists(&conn, "test_migration_temp3").await).to_be_false()?;

    // Verify migration was not recorded
    let applied = runner.applied_migrations().await.unwrap();
    expect(applied.contains(&"20250112_000013".to_string())).to_be_false()?;

    // Clean up
    cleanup_migration_table(&conn, migration_table).await;

    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_pending_migrations() -> Result<(), AssertionError> {
    let migration_table = "_test_migrations_pending";
    let (runner, conn) = create_migration_runner(migration_table).await;

    let migrations = vec![
        Migration::new(
            "20250112_000014".to_string(),
            "migration one".to_string(),
            "CREATE TABLE test_migration_m1 (id SERIAL)".to_string(),
            "DROP TABLE test_migration_m1".to_string(),
        ),
        Migration::new(
            "20250112_000015".to_string(),
            "migration two".to_string(),
            "CREATE TABLE test_migration_m2 (id SERIAL)".to_string(),
            "DROP TABLE test_migration_m2".to_string(),
        ),
        Migration::new(
            "20250112_000016".to_string(),
            "migration three".to_string(),
            "CREATE TABLE test_migration_m3 (id SERIAL)".to_string(),
            "DROP TABLE test_migration_m3".to_string(),
        ),
    ];

    // Initially, all migrations are pending
    let pending = runner.pending_migrations(&migrations).await.unwrap();
    expect(pending.len()).to_equal(&3)?;

    // Apply first migration
    runner.apply(&migrations[0]).await.unwrap();

    // Now only 2 should be pending
    let pending = runner.pending_migrations(&migrations).await.unwrap();
    expect(pending.len()).to_equal(&2)?;
    expect(pending[0].version.as_str()).to_equal(&"20250112_000015")?;
    expect(pending[1].version.as_str()).to_equal(&"20250112_000016")?;

    // Apply all
    runner.migrate(&migrations).await.unwrap();

    // Now nothing should be pending
    let pending = runner.pending_migrations(&migrations).await.unwrap();
    expect(pending.len()).to_equal(&0)?;

    // Clean up
    let pool = conn.pool();
    sqlx::query("DROP TABLE IF EXISTS test_migration_m1 CASCADE")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DROP TABLE IF EXISTS test_migration_m2 CASCADE")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DROP TABLE IF EXISTS test_migration_m3 CASCADE")
        .execute(pool)
        .await
        .unwrap();
    cleanup_migration_table(&conn, migration_table).await;

    Ok(())
}
