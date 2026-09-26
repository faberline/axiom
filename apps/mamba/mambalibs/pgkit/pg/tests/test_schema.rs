//! Integration tests for schema introspection system.
//!
//! These tests verify SchemaInspector operations including:
//! - Complex type introspection (Arrays, JSONB, Enums)
//! - Foreign key introspection with cascade rules
//! - Index introspection
//! - Table listing and existence checking
//!
//! These tests require a PostgreSQL database to be running.
//! Set POSTGRES_URL environment variable or skip with SKIP_INTEGRATION=true
//!
//! Run tests with: cargo test --package cclab-titan --test test_schema -- --ignored

use pg::schema::ColumnType;
use pg::{Connection, PoolConfig, SchemaInspector};
use qc::{expect, AssertionError};

/// Helper to create a test database connection
async fn create_test_connection() -> Connection {
    let uri = std::env::var("POSTGRES_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .unwrap_or_else(|_| "postgresql://localhost/test_db".to_string());

    Connection::new(&uri, PoolConfig::default())
        .await
        .expect("Failed to connect to database")
}

/// Helper to clean up test table
async fn cleanup_table(conn: &Connection, table_name: &str) {
    let pool = conn.pool();
    let sql = format!("DROP TABLE IF EXISTS {} CASCADE", table_name);
    sqlx::query(&sql)
        .execute(pool)
        .await
        .expect("Failed to drop test table");
}

/// Helper to clean up test type
async fn cleanup_type(conn: &Connection, type_name: &str) {
    let pool = conn.pool();
    let sql = format!("DROP TYPE IF EXISTS {} CASCADE", type_name);
    sqlx::query(&sql)
        .execute(pool)
        .await
        .expect("Failed to drop test type");
}

#[tokio::test]
#[ignore]
async fn test_inspector_list_tables() -> Result<(), AssertionError> {
    let conn = create_test_connection().await;
    let inspector = SchemaInspector::new(conn.clone());

    // Create test tables
    let pool = conn.pool();
    sqlx::query("CREATE TABLE test_schema_list_1 (id SERIAL PRIMARY KEY)")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("CREATE TABLE test_schema_list_2 (id SERIAL PRIMARY KEY)")
        .execute(pool)
        .await
        .unwrap();

    // List tables
    let tables = inspector.list_tables(None).await.unwrap();

    // Verify our test tables are in the list
    expect(tables.contains(&"test_schema_list_1".to_string())).to_be_true()?;
    expect(tables.contains(&"test_schema_list_2".to_string())).to_be_true()?;

    // Clean up
    cleanup_table(&conn, "test_schema_list_1").await;
    cleanup_table(&conn, "test_schema_list_2").await;

    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_inspector_table_exists() -> Result<(), AssertionError> {
    let conn = create_test_connection().await;
    let inspector = SchemaInspector::new(conn.clone());

    // Create a test table
    let pool = conn.pool();
    sqlx::query("CREATE TABLE test_schema_exists (id SERIAL PRIMARY KEY)")
        .execute(pool)
        .await
        .unwrap();

    // Verify table exists
    expect(
        inspector
            .table_exists("test_schema_exists", None)
            .await
            .unwrap(),
    )
    .to_be_true()?;

    // Verify non-existent table
    expect(
        inspector
            .table_exists("test_schema_does_not_exist", None)
            .await
            .unwrap(),
    )
    .to_be_false()?;

    // Clean up
    cleanup_table(&conn, "test_schema_exists").await;

    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_inspector_get_columns_with_complex_types() -> Result<(), AssertionError> {
    let conn = create_test_connection().await;
    let inspector = SchemaInspector::new(conn.clone());

    // Create custom enum type
    let pool = conn.pool();
    sqlx::query("CREATE TYPE test_schema_status AS ENUM ('active', 'inactive', 'pending')")
        .execute(pool)
        .await
        .unwrap();

    // Create table with complex types
    sqlx::query(
        "CREATE TABLE test_schema_complex_types (
            id SERIAL PRIMARY KEY,
            tags INTEGER[],
            metadata JSONB,
            status test_schema_status,
            name TEXT NOT NULL
        )",
    )
    .execute(pool)
    .await
    .unwrap();

    // Get columns
    let columns = inspector
        .get_columns("test_schema_complex_types", None)
        .await
        .unwrap();

    // Verify we have all columns
    expect(columns.len()).to_equal(&5)?;

    // Find and verify each column type
    let _tags_col = columns
        .iter()
        .find(|c| c.name == "tags")
        .expect("tags column not found");
    let metadata_col = columns
        .iter()
        .find(|c| c.name == "metadata")
        .expect("metadata column not found");
    let status_col = columns
        .iter()
        .find(|c| c.name == "status")
        .expect("status column not found");
    let name_col = columns
        .iter()
        .find(|c| c.name == "name")
        .expect("name column not found");

    // Verify JSONB type
    expect(metadata_col.data_type == ColumnType::Jsonb).to_be_true()?;

    // Verify custom enum type (should be Custom)
    match &status_col.data_type {
        ColumnType::Custom(type_name) => {
            expect(type_name.contains("test_schema_status")).to_be_true()?;
        }
        _ => {
            return Err(AssertionError::new(
                "Expected Custom type for enum",
                "column_type_check",
            ))
        }
    }

    // Verify TEXT type and NOT NULL constraint
    expect(name_col.data_type == ColumnType::Text).to_be_true()?;
    expect(name_col.nullable).to_be_false()?;

    // Clean up
    cleanup_table(&conn, "test_schema_complex_types").await;
    cleanup_type(&conn, "test_schema_status").await;

    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_inspector_get_foreign_keys_with_cascade() -> Result<(), AssertionError> {
    let conn = create_test_connection().await;
    let inspector = SchemaInspector::new(conn.clone());

    let pool = conn.pool();

    // Create parent table
    sqlx::query(
        "CREATE TABLE test_schema_fk_parent (
            id SERIAL PRIMARY KEY,
            name TEXT NOT NULL
        )",
    )
    .execute(pool)
    .await
    .unwrap();

    // Create child table with different cascade rules
    sqlx::query(
        "CREATE TABLE test_schema_fk_child (
            id SERIAL PRIMARY KEY,
            parent_id INTEGER NOT NULL,
            data TEXT,
            CONSTRAINT fk_parent_cascade
                FOREIGN KEY (parent_id)
                REFERENCES test_schema_fk_parent(id)
                ON DELETE CASCADE
                ON UPDATE RESTRICT
        )",
    )
    .execute(pool)
    .await
    .unwrap();

    // Get foreign keys
    let foreign_keys = inspector
        .get_foreign_keys("test_schema_fk_child", None)
        .await
        .unwrap();

    // Verify we have one foreign key
    expect(foreign_keys.len()).to_equal(&1)?;

    let fk = &foreign_keys[0];

    // Verify foreign key properties
    expect(fk.name.as_str()).to_equal(&"fk_parent_cascade")?;
    expect(fk.columns.len()).to_equal(&1)?;
    expect(fk.columns[0].as_str()).to_equal(&"parent_id")?;
    expect(fk.referenced_table.as_str()).to_equal(&"test_schema_fk_parent")?;
    expect(fk.referenced_columns.len()).to_equal(&1)?;
    expect(fk.referenced_columns[0].as_str()).to_equal(&"id")?;

    // Verify cascade rules
    expect(fk.on_delete.to_uppercase().as_str()).to_equal(&"CASCADE")?;
    expect(fk.on_update.to_uppercase().as_str()).to_equal(&"RESTRICT")?;

    // Clean up (order matters due to FK)
    cleanup_table(&conn, "test_schema_fk_child").await;
    cleanup_table(&conn, "test_schema_fk_parent").await;

    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_inspector_get_indexes() -> Result<(), AssertionError> {
    let conn = create_test_connection().await;
    let inspector = SchemaInspector::new(conn.clone());

    let pool = conn.pool();

    // Create table with indexes
    sqlx::query(
        "CREATE TABLE test_schema_indexes (
            id SERIAL PRIMARY KEY,
            email TEXT NOT NULL,
            username TEXT NOT NULL,
            age INTEGER
        )",
    )
    .execute(pool)
    .await
    .unwrap();

    // Create a unique index
    sqlx::query("CREATE UNIQUE INDEX test_schema_idx_email ON test_schema_indexes(email)")
        .execute(pool)
        .await
        .unwrap();

    // Create a regular btree index
    sqlx::query("CREATE INDEX test_schema_idx_username ON test_schema_indexes(username)")
        .execute(pool)
        .await
        .unwrap();

    // Get indexes
    let indexes = inspector
        .get_indexes("test_schema_indexes", None)
        .await
        .unwrap();

    // Find our test indexes (there will also be a primary key index)
    let email_idx = indexes
        .iter()
        .find(|i| i.name == "test_schema_idx_email")
        .expect("email index not found");
    let username_idx = indexes
        .iter()
        .find(|i| i.name == "test_schema_idx_username")
        .expect("username index not found");

    // Verify email index (unique)
    expect(email_idx.is_unique).to_be_true()?;
    expect(email_idx.index_type.as_str()).to_equal(&"btree")?;
    expect(email_idx.columns.len()).to_equal(&1)?;
    expect(email_idx.columns[0].as_str()).to_equal(&"email")?;

    // Verify username index (non-unique)
    expect(username_idx.is_unique).to_be_false()?;
    expect(username_idx.index_type.as_str()).to_equal(&"btree")?;
    expect(username_idx.columns.len()).to_equal(&1)?;
    expect(username_idx.columns[0].as_str()).to_equal(&"username")?;

    // Clean up
    cleanup_table(&conn, "test_schema_indexes").await;

    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_inspector_inspect_table_full() -> Result<(), AssertionError> {
    let conn = create_test_connection().await;
    let inspector = SchemaInspector::new(conn.clone());

    let pool = conn.pool();

    // Create a comprehensive test table
    sqlx::query(
        "CREATE TABLE test_schema_full (
            id SERIAL PRIMARY KEY,
            email TEXT NOT NULL UNIQUE,
            data JSONB,
            created_at TIMESTAMP DEFAULT NOW()
        )",
    )
    .execute(pool)
    .await
    .unwrap();

    // Create an index
    sqlx::query("CREATE INDEX test_schema_full_idx_email ON test_schema_full(email)")
        .execute(pool)
        .await
        .unwrap();

    // Inspect the full table
    let table_info = inspector
        .inspect_table("test_schema_full", None)
        .await
        .unwrap();

    // Verify table info
    expect(table_info.name.as_str()).to_equal(&"test_schema_full")?;
    expect(table_info.schema.as_str()).to_equal(&"public")?;

    // Verify columns
    expect(table_info.columns.len()).to_equal(&4)?;

    let id_col = table_info
        .columns
        .iter()
        .find(|c| c.name == "id")
        .expect("id column not found");
    expect(id_col.is_primary_key).to_be_true()?;

    let email_col = table_info
        .columns
        .iter()
        .find(|c| c.name == "email")
        .expect("email column not found");
    expect(email_col.is_unique).to_be_true()?;
    expect(email_col.nullable).to_be_false()?;

    let data_col = table_info
        .columns
        .iter()
        .find(|c| c.name == "data")
        .expect("data column not found");
    expect(data_col.data_type == ColumnType::Jsonb).to_be_true()?;
    expect(data_col.nullable).to_be_true()?;

    let created_col = table_info
        .columns
        .iter()
        .find(|c| c.name == "created_at")
        .expect("created_at column not found");
    expect(created_col.default.is_some()).to_be_true()?;

    // Verify indexes exist (at least the one we created)
    let has_our_index = table_info
        .indexes
        .iter()
        .any(|i| i.name == "test_schema_full_idx_email");
    expect(has_our_index).to_be_true()?;

    // Verify no foreign keys
    expect(table_info.foreign_keys.is_empty()).to_be_true()?;

    // Clean up
    cleanup_table(&conn, "test_schema_full").await;

    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_inspector_inspect_nonexistent_table() -> Result<(), AssertionError> {
    let conn = create_test_connection().await;
    let inspector = SchemaInspector::new(conn);

    // Try to inspect a table that doesn't exist
    let result = inspector
        .inspect_table("test_schema_does_not_exist", None)
        .await;

    // Should return an error
    expect(result.is_err()).to_be_true()?;

    Ok(())
}
