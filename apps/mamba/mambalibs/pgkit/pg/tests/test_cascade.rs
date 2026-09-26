//! Integration tests for Cascade Operations.
//!
//! These tests require a PostgreSQL database to be running.
//! Set DATABASE_URL environment variable to customize connection.
//!
//! Run with: cargo test -p cclab-titan --test test_cascade

use pg::{Connection, PoolConfig};

/// Helper to get database URL from environment
fn get_database_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql://localhost/test_db".to_string())
}

/// Helper to cleanup test tables
async fn cleanup_tables(pool: &sqlx::PgPool, tables: &[&str]) -> Result<(), sqlx::Error> {
    for table in tables {
        sqlx::query(&format!("DROP TABLE IF EXISTS {} CASCADE", table))
            .execute(pool)
            .await?;
    }
    Ok(())
}

// =============================================================================
// ON DELETE CASCADE Tests
// =============================================================================

#[tokio::test]
async fn test_cascade_delete_simple() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let parent_table = "test_cascade_parent";
    let child_table = "test_cascade_child";

    cleanup_tables(pool, &[child_table, parent_table]).await?;

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

    // Create child table with ON DELETE CASCADE
    sqlx::query(&format!(
        "CREATE TABLE {} (
            id SERIAL PRIMARY KEY,
            parent_id INTEGER NOT NULL REFERENCES {}(id) ON DELETE CASCADE,
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
    .bind("Parent 1")
    .fetch_one(pool)
    .await?;

    // Insert children
    sqlx::query(&format!(
        "INSERT INTO {} (parent_id, data) VALUES ($1, $2), ($1, $3), ($1, $4)",
        child_table
    ))
    .bind(parent.0)
    .bind("Child 1")
    .bind("Child 2")
    .bind("Child 3")
    .execute(pool)
    .await?;

    // Verify children exist
    let count: (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM {}", child_table))
        .fetch_one(pool)
        .await?;
    assert_eq!(count.0, 3);

    // Delete parent - should cascade to children
    sqlx::query(&format!("DELETE FROM {} WHERE id = $1", parent_table))
        .bind(parent.0)
        .execute(pool)
        .await?;

    // Verify children were deleted
    let count: (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM {}", child_table))
        .fetch_one(pool)
        .await?;
    assert_eq!(count.0, 0);

    cleanup_tables(pool, &[child_table, parent_table]).await?;
    Ok(())
}

#[tokio::test]
async fn test_cascade_delete_multi_level() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let grandparent = "test_cascade_gp";
    let parent = "test_cascade_p";
    let child = "test_cascade_c";

    cleanup_tables(pool, &[child, parent, grandparent]).await?;

    // Create three-level hierarchy
    sqlx::query(&format!(
        "CREATE TABLE {} (id SERIAL PRIMARY KEY, name TEXT NOT NULL)",
        grandparent
    ))
    .execute(pool)
    .await?;

    sqlx::query(&format!(
        "CREATE TABLE {} (
            id SERIAL PRIMARY KEY,
            gp_id INTEGER NOT NULL REFERENCES {}(id) ON DELETE CASCADE,
            name TEXT NOT NULL
        )",
        parent, grandparent
    ))
    .execute(pool)
    .await?;

    sqlx::query(&format!(
        "CREATE TABLE {} (
            id SERIAL PRIMARY KEY,
            p_id INTEGER NOT NULL REFERENCES {}(id) ON DELETE CASCADE,
            name TEXT NOT NULL
        )",
        child, parent
    ))
    .execute(pool)
    .await?;

    // Insert data
    let gp: (i32,) = sqlx::query_as(&format!(
        "INSERT INTO {} (name) VALUES ($1) RETURNING id",
        grandparent
    ))
    .bind("Grandparent")
    .fetch_one(pool)
    .await?;

    let p: (i32,) = sqlx::query_as(&format!(
        "INSERT INTO {} (gp_id, name) VALUES ($1, $2) RETURNING id",
        parent
    ))
    .bind(gp.0)
    .bind("Parent")
    .fetch_one(pool)
    .await?;

    sqlx::query(&format!(
        "INSERT INTO {} (p_id, name) VALUES ($1, $2)",
        child
    ))
    .bind(p.0)
    .bind("Child")
    .execute(pool)
    .await?;

    // Delete grandparent - should cascade through parent to child
    sqlx::query(&format!("DELETE FROM {} WHERE id = $1", grandparent))
        .bind(gp.0)
        .execute(pool)
        .await?;

    // Verify all levels were deleted
    let gp_count: (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM {}", grandparent))
        .fetch_one(pool)
        .await?;
    let p_count: (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM {}", parent))
        .fetch_one(pool)
        .await?;
    let c_count: (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM {}", child))
        .fetch_one(pool)
        .await?;

    assert_eq!(gp_count.0, 0);
    assert_eq!(p_count.0, 0);
    assert_eq!(c_count.0, 0);

    cleanup_tables(pool, &[child, parent, grandparent]).await?;
    Ok(())
}

// =============================================================================
// ON DELETE SET NULL Tests
// =============================================================================

#[tokio::test]
async fn test_cascade_set_null() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let parent_table = "test_setnull_parent";
    let child_table = "test_setnull_child";

    cleanup_tables(pool, &[child_table, parent_table]).await?;

    // Create parent table
    sqlx::query(&format!(
        "CREATE TABLE {} (id SERIAL PRIMARY KEY, name TEXT NOT NULL)",
        parent_table
    ))
    .execute(pool)
    .await?;

    // Create child table with ON DELETE SET NULL
    sqlx::query(&format!(
        "CREATE TABLE {} (
            id SERIAL PRIMARY KEY,
            parent_id INTEGER REFERENCES {}(id) ON DELETE SET NULL,
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

    // Delete parent - child should remain with NULL parent_id
    sqlx::query(&format!("DELETE FROM {} WHERE id = $1", parent_table))
        .bind(parent.0)
        .execute(pool)
        .await?;

    // Verify child still exists with NULL parent_id
    let row: (Option<i32>, String) =
        sqlx::query_as(&format!("SELECT parent_id, data FROM {}", child_table))
            .fetch_one(pool)
            .await?;

    assert!(row.0.is_none());
    assert_eq!(row.1, "Child data");

    cleanup_tables(pool, &[child_table, parent_table]).await?;
    Ok(())
}

// =============================================================================
// ON DELETE RESTRICT Tests
// =============================================================================

#[tokio::test]
async fn test_cascade_restrict() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let parent_table = "test_restrict_parent";
    let child_table = "test_restrict_child";

    cleanup_tables(pool, &[child_table, parent_table]).await?;

    // Create tables with RESTRICT (default behavior)
    sqlx::query(&format!(
        "CREATE TABLE {} (id SERIAL PRIMARY KEY, name TEXT NOT NULL)",
        parent_table
    ))
    .execute(pool)
    .await?;

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

    // Attempt to delete parent - should fail
    let result = sqlx::query(&format!("DELETE FROM {} WHERE id = $1", parent_table))
        .bind(parent.0)
        .execute(pool)
        .await;

    assert!(result.is_err());

    // Verify parent still exists
    let count: (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM {}", parent_table))
        .fetch_one(pool)
        .await?;
    assert_eq!(count.0, 1);

    cleanup_tables(pool, &[child_table, parent_table]).await?;
    Ok(())
}

// =============================================================================
// ON UPDATE CASCADE Tests
// =============================================================================

#[tokio::test]
async fn test_update_cascade() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let parent_table = "test_update_cascade_parent";
    let child_table = "test_update_cascade_child";

    cleanup_tables(pool, &[child_table, parent_table]).await?;

    // Create tables with ON UPDATE CASCADE
    sqlx::query(&format!(
        "CREATE TABLE {} (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL
        )",
        parent_table
    ))
    .execute(pool)
    .await?;

    sqlx::query(&format!(
        "CREATE TABLE {} (
            id SERIAL PRIMARY KEY,
            parent_id INTEGER NOT NULL REFERENCES {}(id) ON UPDATE CASCADE,
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
    .bind("Child data")
    .execute(pool)
    .await?;

    // Update parent's id - should cascade to child
    sqlx::query(&format!(
        "UPDATE {} SET id = $1 WHERE id = $2",
        parent_table
    ))
    .bind(200)
    .bind(100)
    .execute(pool)
    .await?;

    // Verify child's parent_id was updated
    let row: (i32,) = sqlx::query_as(&format!("SELECT parent_id FROM {}", child_table))
        .fetch_one(pool)
        .await?;

    assert_eq!(row.0, 200);

    cleanup_tables(pool, &[child_table, parent_table]).await?;
    Ok(())
}

#[tokio::test]
async fn test_update_set_null() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let parent_table = "test_update_setnull_parent";
    let child_table = "test_update_setnull_child";

    cleanup_tables(pool, &[child_table, parent_table]).await?;

    // Create tables with ON UPDATE SET NULL
    sqlx::query(&format!(
        "CREATE TABLE {} (id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
        parent_table
    ))
    .execute(pool)
    .await?;

    sqlx::query(&format!(
        "CREATE TABLE {} (
            id SERIAL PRIMARY KEY,
            parent_id INTEGER REFERENCES {}(id) ON UPDATE SET NULL,
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
    .bind("Child data")
    .execute(pool)
    .await?;

    // Update parent's id - child's parent_id should become NULL
    sqlx::query(&format!(
        "UPDATE {} SET id = $1 WHERE id = $2",
        parent_table
    ))
    .bind(200)
    .bind(100)
    .execute(pool)
    .await?;

    // Verify child's parent_id is now NULL
    let row: (Option<i32>,) = sqlx::query_as(&format!("SELECT parent_id FROM {}", child_table))
        .fetch_one(pool)
        .await?;

    assert!(row.0.is_none());

    cleanup_tables(pool, &[child_table, parent_table]).await?;
    Ok(())
}

// =============================================================================
// Many-to-Many Cascade Tests
// =============================================================================

#[tokio::test]
async fn test_many_to_many_cascade() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let users_table = "test_m2m_users";
    let groups_table = "test_m2m_groups";
    let join_table = "test_m2m_user_groups";

    cleanup_tables(pool, &[join_table, users_table, groups_table]).await?;

    // Create users and groups tables
    sqlx::query(&format!(
        "CREATE TABLE {} (id SERIAL PRIMARY KEY, name TEXT NOT NULL)",
        users_table
    ))
    .execute(pool)
    .await?;

    sqlx::query(&format!(
        "CREATE TABLE {} (id SERIAL PRIMARY KEY, name TEXT NOT NULL)",
        groups_table
    ))
    .execute(pool)
    .await?;

    // Create junction table with CASCADE on both FKs
    sqlx::query(&format!(
        "CREATE TABLE {} (
            user_id INTEGER NOT NULL REFERENCES {}(id) ON DELETE CASCADE,
            group_id INTEGER NOT NULL REFERENCES {}(id) ON DELETE CASCADE,
            PRIMARY KEY (user_id, group_id)
        )",
        join_table, users_table, groups_table
    ))
    .execute(pool)
    .await?;

    // Insert test data
    let user: (i32,) = sqlx::query_as(&format!(
        "INSERT INTO {} (name) VALUES ($1) RETURNING id",
        users_table
    ))
    .bind("Alice")
    .fetch_one(pool)
    .await?;

    let group1: (i32,) = sqlx::query_as(&format!(
        "INSERT INTO {} (name) VALUES ($1) RETURNING id",
        groups_table
    ))
    .bind("Admins")
    .fetch_one(pool)
    .await?;

    let group2: (i32,) = sqlx::query_as(&format!(
        "INSERT INTO {} (name) VALUES ($1) RETURNING id",
        groups_table
    ))
    .bind("Users")
    .fetch_one(pool)
    .await?;

    // Add user to both groups
    sqlx::query(&format!(
        "INSERT INTO {} (user_id, group_id) VALUES ($1, $2), ($1, $3)",
        join_table
    ))
    .bind(user.0)
    .bind(group1.0)
    .bind(group2.0)
    .execute(pool)
    .await?;

    // Delete user - junction records should cascade
    sqlx::query(&format!("DELETE FROM {} WHERE id = $1", users_table))
        .bind(user.0)
        .execute(pool)
        .await?;

    // Verify junction records were deleted
    let count: (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM {}", join_table))
        .fetch_one(pool)
        .await?;
    assert_eq!(count.0, 0);

    // Groups should still exist
    let group_count: (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM {}", groups_table))
        .fetch_one(pool)
        .await?;
    assert_eq!(group_count.0, 2);

    cleanup_tables(pool, &[join_table, users_table, groups_table]).await?;
    Ok(())
}

// =============================================================================
// Self-Referential Cascade Tests
// =============================================================================

#[tokio::test]
async fn test_self_referential_cascade() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_self_ref_cascade";

    cleanup_tables(pool, &[table]).await?;

    // Create self-referential table (e.g., employee hierarchy)
    sqlx::query(&format!(
        "CREATE TABLE {} (
            id SERIAL PRIMARY KEY,
            name TEXT NOT NULL,
            manager_id INTEGER REFERENCES {}(id) ON DELETE CASCADE
        )",
        table, table
    ))
    .execute(pool)
    .await?;

    // Create hierarchy: CEO -> Manager -> Employee
    let ceo: (i32,) = sqlx::query_as(&format!(
        "INSERT INTO {} (name, manager_id) VALUES ($1, NULL) RETURNING id",
        table
    ))
    .bind("CEO")
    .fetch_one(pool)
    .await?;

    let manager: (i32,) = sqlx::query_as(&format!(
        "INSERT INTO {} (name, manager_id) VALUES ($1, $2) RETURNING id",
        table
    ))
    .bind("Manager")
    .bind(ceo.0)
    .fetch_one(pool)
    .await?;

    sqlx::query(&format!(
        "INSERT INTO {} (name, manager_id) VALUES ($1, $2)",
        table
    ))
    .bind("Employee")
    .bind(manager.0)
    .execute(pool)
    .await?;

    // Delete CEO - should cascade to manager and employee
    sqlx::query(&format!("DELETE FROM {} WHERE id = $1", table))
        .bind(ceo.0)
        .execute(pool)
        .await?;

    // Verify all were deleted
    let count: (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM {}", table))
        .fetch_one(pool)
        .await?;
    assert_eq!(count.0, 0);

    cleanup_tables(pool, &[table]).await?;
    Ok(())
}

#[tokio::test]
async fn test_self_referential_set_null() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_self_ref_setnull";

    cleanup_tables(pool, &[table]).await?;

    // Create self-referential table with SET NULL
    sqlx::query(&format!(
        "CREATE TABLE {} (
            id SERIAL PRIMARY KEY,
            name TEXT NOT NULL,
            manager_id INTEGER REFERENCES {}(id) ON DELETE SET NULL
        )",
        table, table
    ))
    .execute(pool)
    .await?;

    // Create hierarchy
    let ceo: (i32,) = sqlx::query_as(&format!(
        "INSERT INTO {} (name, manager_id) VALUES ($1, NULL) RETURNING id",
        table
    ))
    .bind("CEO")
    .fetch_one(pool)
    .await?;

    sqlx::query(&format!(
        "INSERT INTO {} (name, manager_id) VALUES ($1, $2)",
        table
    ))
    .bind("Manager")
    .bind(ceo.0)
    .execute(pool)
    .await?;

    // Delete CEO - manager's manager_id should become NULL
    sqlx::query(&format!("DELETE FROM {} WHERE id = $1", table))
        .bind(ceo.0)
        .execute(pool)
        .await?;

    // Verify manager still exists but with NULL manager_id
    let row: (String, Option<i32>) =
        sqlx::query_as(&format!("SELECT name, manager_id FROM {}", table))
            .fetch_one(pool)
            .await?;

    assert_eq!(row.0, "Manager");
    assert!(row.1.is_none());

    cleanup_tables(pool, &[table]).await?;
    Ok(())
}

// =============================================================================
// Transaction Cascade Tests
// =============================================================================

#[tokio::test]
async fn test_cascade_in_transaction() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let parent_table = "test_txn_cascade_parent";
    let child_table = "test_txn_cascade_child";

    cleanup_tables(pool, &[child_table, parent_table]).await?;

    // Create tables
    sqlx::query(&format!(
        "CREATE TABLE {} (id SERIAL PRIMARY KEY, name TEXT NOT NULL)",
        parent_table
    ))
    .execute(pool)
    .await?;

    sqlx::query(&format!(
        "CREATE TABLE {} (
            id SERIAL PRIMARY KEY,
            parent_id INTEGER NOT NULL REFERENCES {}(id) ON DELETE CASCADE,
            data TEXT NOT NULL
        )",
        child_table, parent_table
    ))
    .execute(pool)
    .await?;

    // Insert data
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

    // Start transaction
    let mut tx = pool.begin().await?;

    // Delete parent in transaction
    sqlx::query(&format!("DELETE FROM {} WHERE id = $1", parent_table))
        .bind(parent.0)
        .execute(&mut *tx)
        .await?;

    // Verify child is deleted within transaction
    let count: (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM {}", child_table))
        .fetch_one(&mut *tx)
        .await?;
    assert_eq!(count.0, 0);

    // Rollback
    tx.rollback().await?;

    // Verify both parent and child still exist
    let parent_count: (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM {}", parent_table))
        .fetch_one(pool)
        .await?;
    let child_count: (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM {}", child_table))
        .fetch_one(pool)
        .await?;

    assert_eq!(parent_count.0, 1);
    assert_eq!(child_count.0, 1);

    cleanup_tables(pool, &[child_table, parent_table]).await?;
    Ok(())
}

// =============================================================================
// Partial Cascade Tests
// =============================================================================

#[tokio::test]
async fn test_partial_cascade_delete() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let parent_table = "test_partial_cascade_parent";
    let child_table = "test_partial_cascade_child";

    cleanup_tables(pool, &[child_table, parent_table]).await?;

    // Create tables
    sqlx::query(&format!(
        "CREATE TABLE {} (id SERIAL PRIMARY KEY, name TEXT NOT NULL)",
        parent_table
    ))
    .execute(pool)
    .await?;

    sqlx::query(&format!(
        "CREATE TABLE {} (
            id SERIAL PRIMARY KEY,
            parent_id INTEGER NOT NULL REFERENCES {}(id) ON DELETE CASCADE,
            data TEXT NOT NULL
        )",
        child_table, parent_table
    ))
    .execute(pool)
    .await?;

    // Insert multiple parents with children
    let parent1: (i32,) = sqlx::query_as(&format!(
        "INSERT INTO {} (name) VALUES ($1) RETURNING id",
        parent_table
    ))
    .bind("Parent 1")
    .fetch_one(pool)
    .await?;

    let parent2: (i32,) = sqlx::query_as(&format!(
        "INSERT INTO {} (name) VALUES ($1) RETURNING id",
        parent_table
    ))
    .bind("Parent 2")
    .fetch_one(pool)
    .await?;

    sqlx::query(&format!(
        "INSERT INTO {} (parent_id, data) VALUES ($1, $2), ($1, $3)",
        child_table
    ))
    .bind(parent1.0)
    .bind("P1 Child 1")
    .bind("P1 Child 2")
    .execute(pool)
    .await?;

    sqlx::query(&format!(
        "INSERT INTO {} (parent_id, data) VALUES ($1, $2)",
        child_table
    ))
    .bind(parent2.0)
    .bind("P2 Child 1")
    .execute(pool)
    .await?;

    // Delete only parent1
    sqlx::query(&format!("DELETE FROM {} WHERE id = $1", parent_table))
        .bind(parent1.0)
        .execute(pool)
        .await?;

    // Verify only parent1's children were deleted
    let children: Vec<(String,)> = sqlx::query_as(&format!("SELECT data FROM {}", child_table))
        .fetch_all(pool)
        .await?;

    assert_eq!(children.len(), 1);
    assert_eq!(children[0].0, "P2 Child 1");

    cleanup_tables(pool, &[child_table, parent_table]).await?;
    Ok(())
}

// =============================================================================
// ERROR CASES - UPDATE RESTRICT Violation
// =============================================================================

#[tokio::test]
async fn test_update_restrict_violation() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let parent_table = "test_update_restrict_parent";
    let child_table = "test_update_restrict_child";

    cleanup_tables(pool, &[child_table, parent_table]).await?;

    // Create parent with non-serial PK
    sqlx::query(&format!(
        "CREATE TABLE {} (id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
        parent_table
    ))
    .execute(pool)
    .await?;

    // Create child with ON UPDATE RESTRICT
    sqlx::query(&format!(
        "CREATE TABLE {} (
            id SERIAL PRIMARY KEY,
            parent_id INTEGER NOT NULL REFERENCES {}(id) ON UPDATE RESTRICT,
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

    // Try to update parent PK - should fail due to RESTRICT
    let result = sqlx::query(&format!(
        "UPDATE {} SET id = $1 WHERE id = $2",
        parent_table
    ))
    .bind(200)
    .bind(100)
    .execute(pool)
    .await;

    assert!(result.is_err());
    let err_str = result.unwrap_err().to_string().to_lowercase();
    assert!(
        err_str.contains("foreign key") || err_str.contains("violates"),
        "Expected FK violation, got: {}",
        err_str
    );

    cleanup_tables(pool, &[child_table, parent_table]).await?;
    Ok(())
}

// =============================================================================
// EDGE CASES - Deep Cascade Chain
// =============================================================================

#[tokio::test]
async fn test_deep_cascade_chain() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();

    let tables = vec![
        "test_deep_l1",
        "test_deep_l2",
        "test_deep_l3",
        "test_deep_l4",
        "test_deep_l5",
    ];

    // Cleanup in reverse order
    for table in tables.iter().rev() {
        cleanup_tables(pool, &[table]).await?;
    }

    // Create 5-level hierarchy
    sqlx::query(&format!(
        "CREATE TABLE {} (id SERIAL PRIMARY KEY, name TEXT NOT NULL)",
        tables[0]
    ))
    .execute(pool)
    .await?;

    for i in 1..5 {
        sqlx::query(&format!(
            "CREATE TABLE {} (
                id SERIAL PRIMARY KEY,
                parent_id INTEGER NOT NULL REFERENCES {}(id) ON DELETE CASCADE,
                name TEXT NOT NULL
            )",
            tables[i],
            tables[i - 1]
        ))
        .execute(pool)
        .await?;
    }

    // Insert data at each level
    let l1: (i32,) = sqlx::query_as(&format!(
        "INSERT INTO {} (name) VALUES ($1) RETURNING id",
        tables[0]
    ))
    .bind("Level 1")
    .fetch_one(pool)
    .await?;

    let l2: (i32,) = sqlx::query_as(&format!(
        "INSERT INTO {} (parent_id, name) VALUES ($1, $2) RETURNING id",
        tables[1]
    ))
    .bind(l1.0)
    .bind("Level 2")
    .fetch_one(pool)
    .await?;

    let l3: (i32,) = sqlx::query_as(&format!(
        "INSERT INTO {} (parent_id, name) VALUES ($1, $2) RETURNING id",
        tables[2]
    ))
    .bind(l2.0)
    .bind("Level 3")
    .fetch_one(pool)
    .await?;

    let l4: (i32,) = sqlx::query_as(&format!(
        "INSERT INTO {} (parent_id, name) VALUES ($1, $2) RETURNING id",
        tables[3]
    ))
    .bind(l3.0)
    .bind("Level 4")
    .fetch_one(pool)
    .await?;

    sqlx::query(&format!(
        "INSERT INTO {} (parent_id, name) VALUES ($1, $2)",
        tables[4]
    ))
    .bind(l4.0)
    .bind("Level 5")
    .execute(pool)
    .await?;

    // Delete level 1 - should cascade through all levels
    sqlx::query(&format!("DELETE FROM {} WHERE id = $1", tables[0]))
        .bind(l1.0)
        .execute(pool)
        .await?;

    // Verify all levels are empty
    for table in &tables {
        let count: (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM {}", table))
            .fetch_one(pool)
            .await?;
        assert_eq!(count.0, 0, "Table {} should be empty", table);
    }

    // Cleanup
    for table in tables.iter().rev() {
        cleanup_tables(pool, &[table]).await?;
    }

    Ok(())
}

// =============================================================================
// EDGE CASES - Wide Fan-Out Cascade
// =============================================================================

#[tokio::test]
async fn test_wide_fanout_cascade() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let parent_table = "test_fanout_parent";
    let child_table = "test_fanout_child";

    cleanup_tables(pool, &[child_table, parent_table]).await?;

    // Create tables
    sqlx::query(&format!(
        "CREATE TABLE {} (id SERIAL PRIMARY KEY, name TEXT NOT NULL)",
        parent_table
    ))
    .execute(pool)
    .await?;

    sqlx::query(&format!(
        "CREATE TABLE {} (
            id SERIAL PRIMARY KEY,
            parent_id INTEGER NOT NULL REFERENCES {}(id) ON DELETE CASCADE,
            data TEXT NOT NULL
        )",
        child_table, parent_table
    ))
    .execute(pool)
    .await?;

    // Insert parent with many children (wide fan-out)
    let parent: (i32,) = sqlx::query_as(&format!(
        "INSERT INTO {} (name) VALUES ($1) RETURNING id",
        parent_table
    ))
    .bind("Parent")
    .fetch_one(pool)
    .await?;

    // Insert 100 children
    for i in 0..100 {
        sqlx::query(&format!(
            "INSERT INTO {} (parent_id, data) VALUES ($1, $2)",
            child_table
        ))
        .bind(parent.0)
        .bind(format!("Child {}", i))
        .execute(pool)
        .await?;
    }

    // Verify children exist
    let count: (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM {}", child_table))
        .fetch_one(pool)
        .await?;
    assert_eq!(count.0, 100);

    // Delete parent - all 100 children should cascade
    sqlx::query(&format!("DELETE FROM {} WHERE id = $1", parent_table))
        .bind(parent.0)
        .execute(pool)
        .await?;

    // Verify all children deleted
    let count: (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM {}", child_table))
        .fetch_one(pool)
        .await?;
    assert_eq!(count.0, 0);

    cleanup_tables(pool, &[child_table, parent_table]).await?;
    Ok(())
}

// =============================================================================
// EDGE CASES - Concurrent Delete/Update
// =============================================================================

#[tokio::test]
async fn test_concurrent_cascade_delete() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let parent_table = "test_conc_cascade_parent";
    let child_table = "test_conc_cascade_child";

    cleanup_tables(pool, &[child_table, parent_table]).await?;

    // Create tables
    sqlx::query(&format!(
        "CREATE TABLE {} (id SERIAL PRIMARY KEY, name TEXT NOT NULL)",
        parent_table
    ))
    .execute(pool)
    .await?;

    sqlx::query(&format!(
        "CREATE TABLE {} (
            id SERIAL PRIMARY KEY,
            parent_id INTEGER NOT NULL REFERENCES {}(id) ON DELETE CASCADE,
            data TEXT NOT NULL
        )",
        child_table, parent_table
    ))
    .execute(pool)
    .await?;

    // Insert multiple parents with children
    for i in 0..10 {
        let parent: (i32,) = sqlx::query_as(&format!(
            "INSERT INTO {} (name) VALUES ($1) RETURNING id",
            parent_table
        ))
        .bind(format!("Parent {}", i))
        .fetch_one(pool)
        .await?;

        for j in 0..5 {
            sqlx::query(&format!(
                "INSERT INTO {} (parent_id, data) VALUES ($1, $2)",
                child_table
            ))
            .bind(parent.0)
            .bind(format!("Child {}-{}", i, j))
            .execute(pool)
            .await?;
        }
    }

    // Concurrent deletes of all parents
    let mut handles = vec![];
    for i in 0..10 {
        let pool_clone = pool.clone();
        let parent_tbl = parent_table.to_string();
        handles.push(tokio::spawn(async move {
            // Small delay to increase concurrency
            tokio::time::sleep(std::time::Duration::from_millis(i as u64 * 5)).await;
            sqlx::query(&format!("DELETE FROM {} WHERE name = $1", parent_tbl))
                .bind(format!("Parent {}", i))
                .execute(&pool_clone)
                .await
        }));
    }

    // Wait for all deletes
    for handle in handles {
        let _ = handle.await?;
    }

    // Verify all parents and children deleted
    let parent_count: (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM {}", parent_table))
        .fetch_one(pool)
        .await?;
    let child_count: (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM {}", child_table))
        .fetch_one(pool)
        .await?;

    assert_eq!(parent_count.0, 0);
    assert_eq!(child_count.0, 0);

    cleanup_tables(pool, &[child_table, parent_table]).await?;
    Ok(())
}

#[tokio::test]
async fn test_concurrent_parent_delete_child_insert() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let parent_table = "test_conc_del_ins_parent";
    let child_table = "test_conc_del_ins_child";

    cleanup_tables(pool, &[child_table, parent_table]).await?;

    // Create tables
    sqlx::query(&format!(
        "CREATE TABLE {} (id SERIAL PRIMARY KEY, name TEXT NOT NULL)",
        parent_table
    ))
    .execute(pool)
    .await?;

    sqlx::query(&format!(
        "CREATE TABLE {} (
            id SERIAL PRIMARY KEY,
            parent_id INTEGER NOT NULL REFERENCES {}(id) ON DELETE CASCADE,
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

    let parent_id = parent.0;

    // Race: delete parent while trying to insert child
    let pool_clone1 = pool.clone();
    let pool_clone2 = pool.clone();
    let parent_tbl = parent_table.to_string();
    let child_tbl = child_table.to_string();

    let (_delete_result, _insert_result) = tokio::join!(
        async move {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            sqlx::query(&format!("DELETE FROM {} WHERE id = $1", parent_tbl))
                .bind(parent_id)
                .execute(&pool_clone1)
                .await
        },
        async move {
            sqlx::query(&format!(
                "INSERT INTO {} (parent_id, data) VALUES ($1, $2)",
                child_tbl
            ))
            .bind(parent_id)
            .bind("Racing child")
            .execute(&pool_clone2)
            .await
        }
    );

    // One of these might fail due to FK constraint or timing
    // Both might succeed if insert happens before delete
    // Either outcome is valid - we just verify no crash/deadlock

    let parent_count: (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM {}", parent_table))
        .fetch_one(pool)
        .await?;
    let child_count: (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM {}", child_table))
        .fetch_one(pool)
        .await?;

    // If parent deleted, child should also be deleted (or insert failed)
    if parent_count.0 == 0 {
        assert_eq!(child_count.0, 0, "Child should be deleted with parent");
    }

    cleanup_tables(pool, &[child_table, parent_table]).await?;
    Ok(())
}

// =============================================================================
// EDGE CASES - Multiple FKs in Child Table
// =============================================================================

#[tokio::test]
async fn test_multi_fk_cascade() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let users_table = "test_multi_fk_users";
    let projects_table = "test_multi_fk_projects";
    let assignments_table = "test_multi_fk_assignments";

    cleanup_tables(pool, &[assignments_table, users_table, projects_table]).await?;

    // Create users table
    sqlx::query(&format!(
        "CREATE TABLE {} (id SERIAL PRIMARY KEY, name TEXT NOT NULL)",
        users_table
    ))
    .execute(pool)
    .await?;

    // Create projects table
    sqlx::query(&format!(
        "CREATE TABLE {} (id SERIAL PRIMARY KEY, name TEXT NOT NULL)",
        projects_table
    ))
    .execute(pool)
    .await?;

    // Create assignments table with FKs to both
    sqlx::query(&format!(
        "CREATE TABLE {} (
            id SERIAL PRIMARY KEY,
            user_id INTEGER NOT NULL REFERENCES {}(id) ON DELETE CASCADE,
            project_id INTEGER NOT NULL REFERENCES {}(id) ON DELETE CASCADE,
            role TEXT NOT NULL
        )",
        assignments_table, users_table, projects_table
    ))
    .execute(pool)
    .await?;

    // Insert data
    let user1: (i32,) = sqlx::query_as(&format!(
        "INSERT INTO {} (name) VALUES ($1) RETURNING id",
        users_table
    ))
    .bind("Alice")
    .fetch_one(pool)
    .await?;

    let user2: (i32,) = sqlx::query_as(&format!(
        "INSERT INTO {} (name) VALUES ($1) RETURNING id",
        users_table
    ))
    .bind("Bob")
    .fetch_one(pool)
    .await?;

    let proj1: (i32,) = sqlx::query_as(&format!(
        "INSERT INTO {} (name) VALUES ($1) RETURNING id",
        projects_table
    ))
    .bind("Project A")
    .fetch_one(pool)
    .await?;

    let proj2: (i32,) = sqlx::query_as(&format!(
        "INSERT INTO {} (name) VALUES ($1) RETURNING id",
        projects_table
    ))
    .bind("Project B")
    .fetch_one(pool)
    .await?;

    // Create assignments
    sqlx::query(&format!(
        "INSERT INTO {} (user_id, project_id, role) VALUES ($1, $2, $3)",
        assignments_table
    ))
    .bind(user1.0)
    .bind(proj1.0)
    .bind("Developer")
    .execute(pool)
    .await?;

    sqlx::query(&format!(
        "INSERT INTO {} (user_id, project_id, role) VALUES ($1, $2, $3)",
        assignments_table
    ))
    .bind(user1.0)
    .bind(proj2.0)
    .bind("Manager")
    .execute(pool)
    .await?;

    sqlx::query(&format!(
        "INSERT INTO {} (user_id, project_id, role) VALUES ($1, $2, $3)",
        assignments_table
    ))
    .bind(user2.0)
    .bind(proj1.0)
    .bind("Tester")
    .execute(pool)
    .await?;

    // Delete user1 - should delete Alice's assignments only
    sqlx::query(&format!("DELETE FROM {} WHERE id = $1", users_table))
        .bind(user1.0)
        .execute(pool)
        .await?;

    let count: (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM {}", assignments_table))
        .fetch_one(pool)
        .await?;
    assert_eq!(count.0, 1); // Only Bob's assignment remains

    // Delete proj1 - should delete Bob's remaining assignment
    sqlx::query(&format!("DELETE FROM {} WHERE id = $1", projects_table))
        .bind(proj1.0)
        .execute(pool)
        .await?;

    let count: (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM {}", assignments_table))
        .fetch_one(pool)
        .await?;
    assert_eq!(count.0, 0);

    cleanup_tables(pool, &[assignments_table, users_table, projects_table]).await?;
    Ok(())
}

// =============================================================================
// EDGE CASES - Circular Reference (if supported)
// =============================================================================

#[tokio::test]
async fn test_circular_reference_cascade() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_circular_ref";

    cleanup_tables(pool, &[table]).await?;

    // Create table with circular reference (e.g., next_id points to another row)
    sqlx::query(&format!(
        "CREATE TABLE {} (
            id SERIAL PRIMARY KEY,
            name TEXT NOT NULL,
            next_id INTEGER REFERENCES {}(id) ON DELETE SET NULL
        )",
        table, table
    ))
    .execute(pool)
    .await?;

    // Insert circular chain: A -> B -> C -> A
    let a: (i32,) = sqlx::query_as(&format!(
        "INSERT INTO {} (name, next_id) VALUES ($1, NULL) RETURNING id",
        table
    ))
    .bind("A")
    .fetch_one(pool)
    .await?;

    let b: (i32,) = sqlx::query_as(&format!(
        "INSERT INTO {} (name, next_id) VALUES ($1, NULL) RETURNING id",
        table
    ))
    .bind("B")
    .fetch_one(pool)
    .await?;

    let c: (i32,) = sqlx::query_as(&format!(
        "INSERT INTO {} (name, next_id) VALUES ($1, $2) RETURNING id",
        table
    ))
    .bind("C")
    .bind(a.0) // C -> A
    .fetch_one(pool)
    .await?;

    // Update to create chain: A -> B -> C -> A
    sqlx::query(&format!("UPDATE {} SET next_id = $1 WHERE id = $2", table))
        .bind(b.0) // A -> B
        .bind(a.0)
        .execute(pool)
        .await?;

    sqlx::query(&format!("UPDATE {} SET next_id = $1 WHERE id = $2", table))
        .bind(c.0) // B -> C
        .bind(b.0)
        .execute(pool)
        .await?;

    // Delete A - B and C should remain, but their next_ids may be set to NULL
    sqlx::query(&format!("DELETE FROM {} WHERE id = $1", table))
        .bind(a.0)
        .execute(pool)
        .await?;

    // Verify B and C still exist
    let count: (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM {}", table))
        .fetch_one(pool)
        .await?;
    assert_eq!(count.0, 2);

    // C's next_id should be NULL (was pointing to A which is deleted)
    let c_row: (Option<i32>,) =
        sqlx::query_as(&format!("SELECT next_id FROM {} WHERE name = 'C'", table))
            .fetch_one(pool)
            .await?;
    assert!(c_row.0.is_none());

    cleanup_tables(pool, &[table]).await?;
    Ok(())
}
