//! Integration tests for UPSERT operations and RETURNING clauses.
//!
//! These tests require a PostgreSQL database to be running.
//! Set DATABASE_URL environment variable to customize connection.
//!
//! Run with: cargo test -p cclab-titan --test test_upsert

use pg::{Connection, PoolConfig};
use sqlx::Row;

/// Helper to get database URL from environment
fn get_database_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql://localhost/test_db".to_string())
}

/// Helper to setup test table with unique constraint
async fn setup_upsert_table(pool: &sqlx::PgPool, table_name: &str) -> Result<(), sqlx::Error> {
    sqlx::query(&format!("DROP TABLE IF EXISTS {} CASCADE", table_name))
        .execute(pool)
        .await?;

    sqlx::query(&format!(
        "CREATE TABLE {} (
            id BIGSERIAL PRIMARY KEY,
            email TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL,
            login_count INTEGER NOT NULL DEFAULT 0,
            updated_at TIMESTAMP DEFAULT NOW()
        )",
        table_name
    ))
    .execute(pool)
    .await?;

    Ok(())
}

/// Helper to setup composite key table
async fn setup_composite_key_table(
    pool: &sqlx::PgPool,
    table_name: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(&format!("DROP TABLE IF EXISTS {} CASCADE", table_name))
        .execute(pool)
        .await?;

    sqlx::query(&format!(
        "CREATE TABLE {} (
            user_id INTEGER NOT NULL,
            product_id INTEGER NOT NULL,
            quantity INTEGER NOT NULL DEFAULT 1,
            PRIMARY KEY (user_id, product_id)
        )",
        table_name
    ))
    .execute(pool)
    .await?;

    Ok(())
}

/// Helper to cleanup test table
async fn cleanup_table(pool: &sqlx::PgPool, table_name: &str) -> Result<(), sqlx::Error> {
    sqlx::query(&format!("DROP TABLE IF EXISTS {} CASCADE", table_name))
        .execute(pool)
        .await?;
    Ok(())
}

// =============================================================================
// UPSERT Tests - ON CONFLICT DO NOTHING
// =============================================================================

#[tokio::test]
async fn test_upsert_on_conflict_do_nothing() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_upsert_nothing";

    setup_upsert_table(pool, table).await?;

    // Insert initial record
    sqlx::query(&format!(
        "INSERT INTO {} (email, name) VALUES ($1, $2)",
        table
    ))
    .bind("alice@example.com")
    .bind("Alice")
    .execute(pool)
    .await?;

    // Attempt to insert duplicate - should do nothing
    let result = sqlx::query(&format!(
        "INSERT INTO {} (email, name) VALUES ($1, $2) ON CONFLICT (email) DO NOTHING",
        table
    ))
    .bind("alice@example.com")
    .bind("Alice Updated")
    .execute(pool)
    .await?;

    // Should affect 0 rows (conflict, did nothing)
    assert_eq!(result.rows_affected(), 0);

    // Verify original record unchanged
    let row: (String,) = sqlx::query_as(&format!("SELECT name FROM {} WHERE email = $1", table))
        .bind("alice@example.com")
        .fetch_one(pool)
        .await?;

    assert_eq!(row.0, "Alice");

    cleanup_table(pool, table).await?;
    Ok(())
}

#[tokio::test]
async fn test_upsert_insert_when_no_conflict() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_upsert_insert";

    setup_upsert_table(pool, table).await?;

    // Insert with ON CONFLICT - should insert since no conflict
    let result = sqlx::query(&format!(
        "INSERT INTO {} (email, name) VALUES ($1, $2) ON CONFLICT (email) DO NOTHING",
        table
    ))
    .bind("bob@example.com")
    .bind("Bob")
    .execute(pool)
    .await?;

    assert_eq!(result.rows_affected(), 1);

    // Verify record was inserted
    let count: (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM {}", table))
        .fetch_one(pool)
        .await?;

    assert_eq!(count.0, 1);

    cleanup_table(pool, table).await?;
    Ok(())
}

// =============================================================================
// UPSERT Tests - ON CONFLICT DO UPDATE
// =============================================================================

#[tokio::test]
async fn test_upsert_on_conflict_do_update() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_upsert_update";

    setup_upsert_table(pool, table).await?;

    // Insert initial record
    sqlx::query(&format!(
        "INSERT INTO {} (email, name, login_count) VALUES ($1, $2, $3)",
        table
    ))
    .bind("charlie@example.com")
    .bind("Charlie")
    .bind(5)
    .execute(pool)
    .await?;

    // Upsert - should update existing record
    let result = sqlx::query(&format!(
        "INSERT INTO {} (email, name, login_count) VALUES ($1, $2, $3)
         ON CONFLICT (email) DO UPDATE SET
            name = EXCLUDED.name,
            login_count = {}.login_count + 1",
        table, table
    ))
    .bind("charlie@example.com")
    .bind("Charlie Updated")
    .bind(0)
    .execute(pool)
    .await?;

    assert_eq!(result.rows_affected(), 1);

    // Verify record was updated
    let row: (String, i32) = sqlx::query_as(&format!(
        "SELECT name, login_count FROM {} WHERE email = $1",
        table
    ))
    .bind("charlie@example.com")
    .fetch_one(pool)
    .await?;

    assert_eq!(row.0, "Charlie Updated");
    assert_eq!(row.1, 6); // 5 + 1

    cleanup_table(pool, table).await?;
    Ok(())
}

#[tokio::test]
async fn test_upsert_with_excluded_values() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_upsert_excluded";

    setup_upsert_table(pool, table).await?;

    // Insert initial record
    sqlx::query(&format!(
        "INSERT INTO {} (email, name) VALUES ($1, $2)",
        table
    ))
    .bind("dave@example.com")
    .bind("Dave")
    .execute(pool)
    .await?;

    // Upsert using EXCLUDED to reference new values
    sqlx::query(&format!(
        "INSERT INTO {} (email, name) VALUES ($1, $2)
         ON CONFLICT (email) DO UPDATE SET name = EXCLUDED.name",
        table
    ))
    .bind("dave@example.com")
    .bind("David")
    .execute(pool)
    .await?;

    // Verify name was updated to EXCLUDED value
    let row: (String,) = sqlx::query_as(&format!("SELECT name FROM {} WHERE email = $1", table))
        .bind("dave@example.com")
        .fetch_one(pool)
        .await?;

    assert_eq!(row.0, "David");

    cleanup_table(pool, table).await?;
    Ok(())
}

#[tokio::test]
async fn test_upsert_composite_key() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_upsert_composite";

    setup_composite_key_table(pool, table).await?;

    // Insert initial record
    sqlx::query(&format!(
        "INSERT INTO {} (user_id, product_id, quantity) VALUES ($1, $2, $3)",
        table
    ))
    .bind(1)
    .bind(100)
    .bind(2)
    .execute(pool)
    .await?;

    // Upsert with composite key conflict
    sqlx::query(&format!(
        "INSERT INTO {} (user_id, product_id, quantity) VALUES ($1, $2, $3)
         ON CONFLICT (user_id, product_id) DO UPDATE SET
            quantity = {}.quantity + EXCLUDED.quantity",
        table, table
    ))
    .bind(1)
    .bind(100)
    .bind(3)
    .execute(pool)
    .await?;

    // Verify quantity was updated
    let row: (i32,) = sqlx::query_as(&format!(
        "SELECT quantity FROM {} WHERE user_id = $1 AND product_id = $2",
        table
    ))
    .bind(1)
    .bind(100)
    .fetch_one(pool)
    .await?;

    assert_eq!(row.0, 5); // 2 + 3

    cleanup_table(pool, table).await?;
    Ok(())
}

#[tokio::test]
async fn test_upsert_with_where_clause() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_upsert_where";

    setup_upsert_table(pool, table).await?;

    // Insert initial record with login_count = 10
    sqlx::query(&format!(
        "INSERT INTO {} (email, name, login_count) VALUES ($1, $2, $3)",
        table
    ))
    .bind("eve@example.com")
    .bind("Eve")
    .bind(10)
    .execute(pool)
    .await?;

    // Upsert with WHERE clause - should NOT update (login_count > 5)
    let result = sqlx::query(&format!(
        "INSERT INTO {} (email, name, login_count) VALUES ($1, $2, $3)
         ON CONFLICT (email) DO UPDATE SET name = EXCLUDED.name
         WHERE {}.login_count <= 5",
        table, table
    ))
    .bind("eve@example.com")
    .bind("Eve Updated")
    .bind(0)
    .execute(pool)
    .await?;

    // Should affect 0 rows (WHERE condition not met)
    assert_eq!(result.rows_affected(), 0);

    // Verify name unchanged
    let row: (String,) = sqlx::query_as(&format!("SELECT name FROM {} WHERE email = $1", table))
        .bind("eve@example.com")
        .fetch_one(pool)
        .await?;

    assert_eq!(row.0, "Eve");

    cleanup_table(pool, table).await?;
    Ok(())
}

// =============================================================================
// RETURNING Clause Tests
// =============================================================================

#[tokio::test]
async fn test_insert_returning_all() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_returning_all";

    setup_upsert_table(pool, table).await?;

    // Insert with RETURNING *
    let row = sqlx::query(&format!(
        "INSERT INTO {} (email, name) VALUES ($1, $2) RETURNING *",
        table
    ))
    .bind("frank@example.com")
    .bind("Frank")
    .fetch_one(pool)
    .await?;

    let id: i64 = row.get("id");
    let email: String = row.get("email");
    let name: String = row.get("name");

    assert!(id > 0);
    assert_eq!(email, "frank@example.com");
    assert_eq!(name, "Frank");

    cleanup_table(pool, table).await?;
    Ok(())
}

#[tokio::test]
async fn test_insert_returning_specific_columns() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_returning_cols";

    setup_upsert_table(pool, table).await?;

    // Insert with RETURNING specific columns
    let row: (i64, String) = sqlx::query_as(&format!(
        "INSERT INTO {} (email, name) VALUES ($1, $2) RETURNING id, email",
        table
    ))
    .bind("grace@example.com")
    .bind("Grace")
    .fetch_one(pool)
    .await?;

    assert!(row.0 > 0);
    assert_eq!(row.1, "grace@example.com");

    cleanup_table(pool, table).await?;
    Ok(())
}

#[tokio::test]
async fn test_update_returning() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_update_returning";

    setup_upsert_table(pool, table).await?;

    // Insert record
    sqlx::query(&format!(
        "INSERT INTO {} (email, name) VALUES ($1, $2)",
        table
    ))
    .bind("henry@example.com")
    .bind("Henry")
    .execute(pool)
    .await?;

    // Update with RETURNING
    let row = sqlx::query(&format!(
        "UPDATE {} SET name = $1 WHERE email = $2 RETURNING *",
        table
    ))
    .bind("Henry Updated")
    .bind("henry@example.com")
    .fetch_one(pool)
    .await?;

    let name: String = row.get("name");
    assert_eq!(name, "Henry Updated");

    cleanup_table(pool, table).await?;
    Ok(())
}

#[tokio::test]
async fn test_delete_returning() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_delete_returning";

    setup_upsert_table(pool, table).await?;

    // Insert records
    sqlx::query(&format!(
        "INSERT INTO {} (email, name) VALUES ($1, $2), ($3, $4)",
        table
    ))
    .bind("ivan@example.com")
    .bind("Ivan")
    .bind("jane@example.com")
    .bind("Jane")
    .execute(pool)
    .await?;

    // Delete with RETURNING
    let rows: Vec<(String, String)> = sqlx::query_as(&format!(
        "DELETE FROM {} WHERE name LIKE 'I%' RETURNING email, name",
        table
    ))
    .fetch_all(pool)
    .await?;

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].0, "ivan@example.com");
    assert_eq!(rows[0].1, "Ivan");

    // Verify Ivan was deleted, Jane remains
    let count: (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM {}", table))
        .fetch_one(pool)
        .await?;

    assert_eq!(count.0, 1);

    cleanup_table(pool, table).await?;
    Ok(())
}

#[tokio::test]
async fn test_upsert_returning() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_upsert_returning";

    setup_upsert_table(pool, table).await?;

    // Insert initial record
    sqlx::query(&format!(
        "INSERT INTO {} (email, name, login_count) VALUES ($1, $2, $3)",
        table
    ))
    .bind("kate@example.com")
    .bind("Kate")
    .bind(5)
    .execute(pool)
    .await?;

    // Upsert with RETURNING
    let row = sqlx::query(&format!(
        "INSERT INTO {} (email, name, login_count) VALUES ($1, $2, $3)
         ON CONFLICT (email) DO UPDATE SET login_count = {}.login_count + 1
         RETURNING id, email, login_count",
        table, table
    ))
    .bind("kate@example.com")
    .bind("Kate")
    .bind(0)
    .fetch_one(pool)
    .await?;

    let login_count: i32 = row.get("login_count");
    assert_eq!(login_count, 6);

    cleanup_table(pool, table).await?;
    Ok(())
}

// =============================================================================
// Recursive CTE Tests
// =============================================================================

#[tokio::test]
async fn test_recursive_cte_simple() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();

    // Recursive CTE to generate numbers 1-5
    let rows: Vec<(i32,)> = sqlx::query_as(
        "WITH RECURSIVE nums AS (
            SELECT 1 AS n
            UNION ALL
            SELECT n + 1 FROM nums WHERE n < 5
        )
        SELECT n FROM nums",
    )
    .fetch_all(pool)
    .await?;

    assert_eq!(rows.len(), 5);
    assert_eq!(rows[0].0, 1);
    assert_eq!(rows[4].0, 5);

    Ok(())
}

#[tokio::test]
async fn test_recursive_cte_hierarchy() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_hierarchy";

    // Create hierarchy table
    sqlx::query(&format!("DROP TABLE IF EXISTS {} CASCADE", table))
        .execute(pool)
        .await?;

    sqlx::query(&format!(
        "CREATE TABLE {} (
            id SERIAL PRIMARY KEY,
            name TEXT NOT NULL,
            parent_id INTEGER REFERENCES {}(id)
        )",
        table, table
    ))
    .execute(pool)
    .await?;

    // Insert hierarchy: CEO -> VP -> Manager -> Developer
    sqlx::query(&format!(
        "INSERT INTO {} (id, name, parent_id) VALUES
         (1, 'CEO', NULL),
         (2, 'VP', 1),
         (3, 'Manager', 2),
         (4, 'Developer', 3)",
        table
    ))
    .execute(pool)
    .await?;

    // Recursive CTE to get all descendants of CEO
    let rows: Vec<(i32, String, i32)> = sqlx::query_as(&format!(
        "WITH RECURSIVE org_tree AS (
            SELECT id, name, 0 as depth FROM {} WHERE parent_id IS NULL
            UNION ALL
            SELECT e.id, e.name, ot.depth + 1
            FROM {} e
            JOIN org_tree ot ON e.parent_id = ot.id
        )
        SELECT id, name, depth FROM org_tree ORDER BY depth",
        table, table
    ))
    .fetch_all(pool)
    .await?;

    assert_eq!(rows.len(), 4);
    assert_eq!(rows[0].1, "CEO");
    assert_eq!(rows[0].2, 0);
    assert_eq!(rows[3].1, "Developer");
    assert_eq!(rows[3].2, 3);

    cleanup_table(pool, table).await?;
    Ok(())
}

// =============================================================================
// Bulk UPSERT Tests
// =============================================================================

#[tokio::test]
async fn test_bulk_upsert() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_bulk_upsert";

    setup_upsert_table(pool, table).await?;

    // Insert multiple records with potential conflicts
    let result = sqlx::query(&format!(
        "INSERT INTO {} (email, name, login_count) VALUES
         ($1, $2, $3),
         ($4, $5, $6),
         ($7, $8, $9)
         ON CONFLICT (email) DO UPDATE SET
            name = EXCLUDED.name,
            login_count = {}.login_count + EXCLUDED.login_count",
        table, table
    ))
    .bind("user1@example.com")
    .bind("User 1")
    .bind(1)
    .bind("user2@example.com")
    .bind("User 2")
    .bind(2)
    .bind("user3@example.com")
    .bind("User 3")
    .bind(3)
    .execute(pool)
    .await?;

    assert_eq!(result.rows_affected(), 3);

    // Run again - should update all
    let result = sqlx::query(&format!(
        "INSERT INTO {} (email, name, login_count) VALUES
         ($1, $2, $3),
         ($4, $5, $6),
         ($7, $8, $9)
         ON CONFLICT (email) DO UPDATE SET
            name = EXCLUDED.name,
            login_count = {}.login_count + EXCLUDED.login_count",
        table, table
    ))
    .bind("user1@example.com")
    .bind("User 1 Updated")
    .bind(10)
    .bind("user2@example.com")
    .bind("User 2 Updated")
    .bind(20)
    .bind("user3@example.com")
    .bind("User 3 Updated")
    .bind(30)
    .execute(pool)
    .await?;

    assert_eq!(result.rows_affected(), 3);

    // Verify counts were accumulated
    let rows: Vec<(String, i32)> = sqlx::query_as(&format!(
        "SELECT email, login_count FROM {} ORDER BY email",
        table
    ))
    .fetch_all(pool)
    .await?;

    assert_eq!(rows[0].1, 11); // 1 + 10
    assert_eq!(rows[1].1, 22); // 2 + 20
    assert_eq!(rows[2].1, 33); // 3 + 30

    cleanup_table(pool, table).await?;
    Ok(())
}

// =============================================================================
// ERROR CASES - Constraint Violations During UPSERT
// =============================================================================

#[tokio::test]
async fn test_upsert_not_null_violation() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_upsert_not_null";

    setup_upsert_table(pool, table).await?;

    // Attempt to insert NULL into NOT NULL column
    let result = sqlx::query(&format!(
        "INSERT INTO {} (email, name) VALUES ($1, $2)
         ON CONFLICT (email) DO NOTHING",
        table
    ))
    .bind("test@example.com")
    .bind(Option::<String>::None) // NULL for NOT NULL column
    .execute(pool)
    .await;

    assert!(result.is_err());
    let err_str = result.unwrap_err().to_string().to_lowercase();
    assert!(
        err_str.contains("null") || err_str.contains("not null"),
        "Expected NOT NULL violation, got: {}",
        err_str
    );

    cleanup_table(pool, table).await?;
    Ok(())
}

#[tokio::test]
async fn test_upsert_check_constraint_violation() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_upsert_check";

    cleanup_table(pool, table).await?;

    // Create table with CHECK constraint
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

    // Attempt upsert with invalid age
    let result = sqlx::query(&format!(
        "INSERT INTO {} (email, age) VALUES ($1, $2)
         ON CONFLICT (email) DO UPDATE SET age = EXCLUDED.age",
        table
    ))
    .bind("test@example.com")
    .bind(-5) // Violates CHECK constraint
    .execute(pool)
    .await;

    assert!(result.is_err());
    let err_str = result.unwrap_err().to_string().to_lowercase();
    assert!(
        err_str.contains("check") || err_str.contains("violates"),
        "Expected CHECK violation, got: {}",
        err_str
    );

    cleanup_table(pool, table).await?;
    Ok(())
}

#[tokio::test]
async fn test_upsert_fk_violation() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let parent_table = "test_upsert_fk_parent";
    let child_table = "test_upsert_fk_child";

    cleanup_table(pool, child_table).await?;
    cleanup_table(pool, parent_table).await?;

    // Create parent and child tables
    sqlx::query(&format!(
        "CREATE TABLE {} (id SERIAL PRIMARY KEY, name TEXT NOT NULL)",
        parent_table
    ))
    .execute(pool)
    .await?;

    sqlx::query(&format!(
        "CREATE TABLE {} (
            id SERIAL PRIMARY KEY,
            email TEXT NOT NULL UNIQUE,
            parent_id INTEGER NOT NULL REFERENCES {}(id)
        )",
        child_table, parent_table
    ))
    .execute(pool)
    .await?;

    // Attempt upsert with non-existent parent
    let result = sqlx::query(&format!(
        "INSERT INTO {} (email, parent_id) VALUES ($1, $2)
         ON CONFLICT (email) DO UPDATE SET parent_id = EXCLUDED.parent_id",
        child_table
    ))
    .bind("test@example.com")
    .bind(99999) // Non-existent parent
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
async fn test_upsert_invalid_conflict_target() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_upsert_invalid_conflict";

    setup_upsert_table(pool, table).await?;

    // Attempt ON CONFLICT on non-unique column (name is not unique)
    let result = sqlx::query(&format!(
        "INSERT INTO {} (email, name) VALUES ($1, $2)
         ON CONFLICT (name) DO NOTHING",
        table
    ))
    .bind("test@example.com")
    .bind("Test User")
    .execute(pool)
    .await;

    // Should fail - no unique constraint on 'name'
    assert!(result.is_err());

    cleanup_table(pool, table).await?;
    Ok(())
}

// =============================================================================
// EDGE CASES - NULL Handling
// =============================================================================

#[tokio::test]
async fn test_upsert_nullable_unique_column() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_upsert_nullable_unique";

    cleanup_table(pool, table).await?;

    // Create table with nullable unique column
    sqlx::query(&format!(
        "CREATE TABLE {} (
            id SERIAL PRIMARY KEY,
            code TEXT UNIQUE,
            data TEXT NOT NULL
        )",
        table
    ))
    .execute(pool)
    .await?;

    // Insert multiple NULLs - should work (NULL != NULL in unique)
    sqlx::query(&format!(
        "INSERT INTO {} (code, data) VALUES (NULL, $1)",
        table
    ))
    .bind("Data 1")
    .execute(pool)
    .await?;

    sqlx::query(&format!(
        "INSERT INTO {} (code, data) VALUES (NULL, $1)",
        table
    ))
    .bind("Data 2")
    .execute(pool)
    .await?;

    // Both should exist
    let count: (i64,) = sqlx::query_as(&format!(
        "SELECT COUNT(*) FROM {} WHERE code IS NULL",
        table
    ))
    .fetch_one(pool)
    .await?;
    assert_eq!(count.0, 2);

    // Upsert with non-null value should work
    sqlx::query(&format!(
        "INSERT INTO {} (code, data) VALUES ($1, $2)
         ON CONFLICT (code) DO UPDATE SET data = EXCLUDED.data",
        table
    ))
    .bind("CODE1")
    .bind("Data 3")
    .execute(pool)
    .await?;

    // Upsert same code - should update
    sqlx::query(&format!(
        "INSERT INTO {} (code, data) VALUES ($1, $2)
         ON CONFLICT (code) DO UPDATE SET data = EXCLUDED.data",
        table
    ))
    .bind("CODE1")
    .bind("Data 3 Updated")
    .execute(pool)
    .await?;

    let row: (String,) = sqlx::query_as(&format!("SELECT data FROM {} WHERE code = $1", table))
        .bind("CODE1")
        .fetch_one(pool)
        .await?;
    assert_eq!(row.0, "Data 3 Updated");

    cleanup_table(pool, table).await?;
    Ok(())
}

#[tokio::test]
async fn test_upsert_composite_key_with_null() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_upsert_composite_null";

    cleanup_table(pool, table).await?;

    // Create table with composite unique on nullable columns
    sqlx::query(&format!(
        "CREATE TABLE {} (
            id SERIAL PRIMARY KEY,
            tenant_id INTEGER,
            user_id INTEGER,
            data TEXT NOT NULL,
            UNIQUE (tenant_id, user_id)
        )",
        table
    ))
    .execute(pool)
    .await?;

    // Insert with NULL in composite key
    sqlx::query(&format!(
        "INSERT INTO {} (tenant_id, user_id, data) VALUES ($1, $2, $3)",
        table
    ))
    .bind(Option::<i32>::None)
    .bind(1)
    .bind("Data 1")
    .execute(pool)
    .await?;

    // Insert another with same NULL - should work (NULL != NULL)
    sqlx::query(&format!(
        "INSERT INTO {} (tenant_id, user_id, data) VALUES ($1, $2, $3)",
        table
    ))
    .bind(Option::<i32>::None)
    .bind(1)
    .bind("Data 2")
    .execute(pool)
    .await?;

    let count: (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM {}", table))
        .fetch_one(pool)
        .await?;
    assert_eq!(count.0, 2);

    cleanup_table(pool, table).await?;
    Ok(())
}

// =============================================================================
// EDGE CASES - Empty and Boundary Values
// =============================================================================

#[tokio::test]
async fn test_upsert_empty_string() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_upsert_empty_string";

    setup_upsert_table(pool, table).await?;

    // Insert with empty string (valid, not NULL)
    sqlx::query(&format!(
        "INSERT INTO {} (email, name) VALUES ($1, $2)
         ON CONFLICT (email) DO UPDATE SET name = EXCLUDED.name",
        table
    ))
    .bind("empty@example.com")
    .bind("") // Empty string
    .execute(pool)
    .await?;

    let row: (String,) = sqlx::query_as(&format!("SELECT name FROM {} WHERE email = $1", table))
        .bind("empty@example.com")
        .fetch_one(pool)
        .await?;
    assert_eq!(row.0, "");

    cleanup_table(pool, table).await?;
    Ok(())
}

#[tokio::test]
async fn test_upsert_very_long_string() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_upsert_long_string";

    setup_upsert_table(pool, table).await?;

    // Insert with very long string
    let long_name = "A".repeat(10000);
    sqlx::query(&format!(
        "INSERT INTO {} (email, name) VALUES ($1, $2)
         ON CONFLICT (email) DO UPDATE SET name = EXCLUDED.name",
        table
    ))
    .bind("long@example.com")
    .bind(&long_name)
    .execute(pool)
    .await?;

    let row: (String,) = sqlx::query_as(&format!("SELECT name FROM {} WHERE email = $1", table))
        .bind("long@example.com")
        .fetch_one(pool)
        .await?;
    assert_eq!(row.0.len(), 10000);

    cleanup_table(pool, table).await?;
    Ok(())
}

#[tokio::test]
async fn test_upsert_special_characters() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_upsert_special_chars";

    setup_upsert_table(pool, table).await?;

    // Test various special characters
    let special_names = vec![
        "O'Brien",
        "Smith\"Jones",
        "Test\nNewline",
        "Tab\tCharacter",
        "Unicode: 中文 日本語 한국어",
        "Emoji: 🚀🎉",
        "SQL: '; DROP TABLE users; --",
        "Backslash: \\test\\path",
    ];

    for (i, name) in special_names.iter().enumerate() {
        let email = format!("special{}@example.com", i);
        sqlx::query(&format!(
            "INSERT INTO {} (email, name) VALUES ($1, $2)
             ON CONFLICT (email) DO UPDATE SET name = EXCLUDED.name",
            table
        ))
        .bind(&email)
        .bind(*name)
        .execute(pool)
        .await?;

        let row: (String,) =
            sqlx::query_as(&format!("SELECT name FROM {} WHERE email = $1", table))
                .bind(&email)
                .fetch_one(pool)
                .await?;
        assert_eq!(row.0, *name);
    }

    cleanup_table(pool, table).await?;
    Ok(())
}

// =============================================================================
// EDGE CASES - Concurrent Upserts (Race Conditions)
// =============================================================================

#[tokio::test]
async fn test_upsert_concurrent_same_key() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_upsert_concurrent";

    setup_upsert_table(pool, table).await?;

    // Concurrent upserts to same key
    let mut handles = vec![];
    for i in 0..10 {
        let pool_clone = pool.clone();
        let table_name = table.to_string();
        handles.push(tokio::spawn(async move {
            sqlx::query(&format!(
                "INSERT INTO {} (email, name, login_count) VALUES ($1, $2, $3)
                 ON CONFLICT (email) DO UPDATE SET
                    login_count = {}.login_count + 1",
                table_name, table_name
            ))
            .bind("concurrent@example.com")
            .bind(format!("User {}", i))
            .bind(1)
            .execute(&pool_clone)
            .await
        }));
    }

    // Wait for all to complete
    for handle in handles {
        let _ = handle.await?;
    }

    // Should have exactly one record
    let row: (i64, i32) = sqlx::query_as(&format!(
        "SELECT COUNT(*), MAX(login_count) FROM {} WHERE email = $1",
        table
    ))
    .bind("concurrent@example.com")
    .fetch_one(pool)
    .await?;

    assert_eq!(row.0, 1, "Should have exactly one record");
    // login_count should be incremented (exact value depends on race)
    assert!(row.1 >= 1, "login_count should be at least 1");

    cleanup_table(pool, table).await?;
    Ok(())
}

#[tokio::test]
async fn test_upsert_concurrent_different_keys() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_upsert_concurrent_diff";

    setup_upsert_table(pool, table).await?;

    // Concurrent upserts to different keys
    let mut handles = vec![];
    for i in 0..20 {
        let pool_clone = pool.clone();
        let table_name = table.to_string();
        handles.push(tokio::spawn(async move {
            sqlx::query(&format!(
                "INSERT INTO {} (email, name) VALUES ($1, $2)
                 ON CONFLICT (email) DO UPDATE SET name = EXCLUDED.name",
                table_name
            ))
            .bind(format!("user{}@example.com", i))
            .bind(format!("User {}", i))
            .execute(&pool_clone)
            .await
        }));
    }

    // Wait for all to complete
    for handle in handles {
        handle.await??;
    }

    // Should have 20 records
    let count: (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM {}", table))
        .fetch_one(pool)
        .await?;
    assert_eq!(count.0, 20);

    cleanup_table(pool, table).await?;
    Ok(())
}

// =============================================================================
// ERROR CASES - RETURNING Type Mismatch
// =============================================================================

#[tokio::test]
async fn test_returning_wrong_type() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_returning_type";

    setup_upsert_table(pool, table).await?;

    // Insert and return string column as wrong type
    let result: Result<(i32,), _> = sqlx::query_as(&format!(
        "INSERT INTO {} (email, name) VALUES ($1, $2) RETURNING name",
        table
    ))
    .bind("test@example.com")
    .bind("Test User")
    .fetch_one(pool)
    .await;

    // Should fail - name is TEXT, not INT
    assert!(result.is_err());

    cleanup_table(pool, table).await?;
    Ok(())
}

// =============================================================================
// EDGE CASES - DO UPDATE with WHERE filtering all rows
// =============================================================================

#[tokio::test]
async fn test_upsert_where_filters_all() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_upsert_where_filter";

    setup_upsert_table(pool, table).await?;

    // Insert initial record with high login_count
    sqlx::query(&format!(
        "INSERT INTO {} (email, name, login_count) VALUES ($1, $2, $3)",
        table
    ))
    .bind("filter@example.com")
    .bind("Original")
    .bind(100)
    .execute(pool)
    .await?;

    // Upsert with WHERE that never matches (login_count > 100 is false)
    let result = sqlx::query(&format!(
        "INSERT INTO {} (email, name, login_count) VALUES ($1, $2, $3)
         ON CONFLICT (email) DO UPDATE SET name = EXCLUDED.name
         WHERE {}.login_count > 100",
        table, table
    ))
    .bind("filter@example.com")
    .bind("Updated")
    .bind(0)
    .execute(pool)
    .await?;

    // Should affect 0 rows
    assert_eq!(result.rows_affected(), 0);

    // Original should be unchanged
    let row: (String, i32) = sqlx::query_as(&format!(
        "SELECT name, login_count FROM {} WHERE email = $1",
        table
    ))
    .bind("filter@example.com")
    .fetch_one(pool)
    .await?;
    assert_eq!(row.0, "Original");
    assert_eq!(row.1, 100);

    cleanup_table(pool, table).await?;
    Ok(())
}
