//! ORM-level integration tests.
//!
//! Tests the ORM API as exposed to Python, covering QueryBuilder,
//! Transaction, and Connection operations through the ORM layer.
//!
//! These tests simulate how Python users would interact with the ORM.
//!
//! Run with: cargo test -p cclab-titan --test test_orm

use pg::{
    BulkConfig, BulkExecutor, Connection, ExtractedValue, IsolationLevel, JoinCondition, Operator,
    OrderDirection, PoolConfig, QueryBuilder, Transaction, WindowSpec,
};
use std::collections::HashMap;

fn get_database_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql://localhost/test_db".to_string())
}

async fn setup_test_db(pool: &sqlx::PgPool) -> Result<(), sqlx::Error> {
    // Create test tables
    sqlx::query("DROP TABLE IF EXISTS orm_orders CASCADE")
        .execute(pool)
        .await?;
    sqlx::query("DROP TABLE IF EXISTS orm_products CASCADE")
        .execute(pool)
        .await?;
    sqlx::query("DROP TABLE IF EXISTS orm_users CASCADE")
        .execute(pool)
        .await?;

    sqlx::query(
        r#"
        CREATE TABLE orm_users (
            id SERIAL PRIMARY KEY,
            name TEXT NOT NULL,
            email TEXT UNIQUE NOT NULL,
            age INTEGER,
            active BOOLEAN DEFAULT true,
            created_at TIMESTAMP DEFAULT NOW()
        )
    "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE orm_products (
            id SERIAL PRIMARY KEY,
            name TEXT NOT NULL,
            price NUMERIC(10,2) NOT NULL,
            category TEXT,
            stock INTEGER DEFAULT 0
        )
    "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE orm_orders (
            id SERIAL PRIMARY KEY,
            user_id INTEGER REFERENCES orm_users(id),
            product_id INTEGER REFERENCES orm_products(id),
            quantity INTEGER NOT NULL,
            total NUMERIC(10,2),
            status TEXT DEFAULT 'pending',
            created_at TIMESTAMP DEFAULT NOW()
        )
    "#,
    )
    .execute(pool)
    .await?;

    // Insert test data
    sqlx::query(
        r#"
        INSERT INTO orm_users (name, email, age, active) VALUES
        ('Alice', 'alice@example.com', 30, true),
        ('Bob', 'bob@example.com', 25, true),
        ('Charlie', 'charlie@example.com', 35, false),
        ('Diana', 'diana@example.com', 28, true)
    "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO orm_products (name, price, category, stock) VALUES
        ('Laptop', 999.99, 'Electronics', 50),
        ('Phone', 699.99, 'Electronics', 100),
        ('Book', 29.99, 'Books', 200),
        ('Headphones', 149.99, 'Electronics', 75)
    "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO orm_orders (user_id, product_id, quantity, total, status) VALUES
        (1, 1, 1, 999.99, 'completed'),
        (1, 2, 2, 1399.98, 'completed'),
        (2, 3, 5, 149.95, 'pending'),
        (3, 4, 1, 149.99, 'shipped'),
        (4, 1, 1, 999.99, 'completed'),
        (4, 3, 3, 89.97, 'pending')
    "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

async fn teardown_test_db(pool: &sqlx::PgPool) -> Result<(), sqlx::Error> {
    sqlx::query("DROP TABLE IF EXISTS orm_orders CASCADE")
        .execute(pool)
        .await?;
    sqlx::query("DROP TABLE IF EXISTS orm_products CASCADE")
        .execute(pool)
        .await?;
    sqlx::query("DROP TABLE IF EXISTS orm_users CASCADE")
        .execute(pool)
        .await?;
    Ok(())
}

// =============================================================================
// QueryBuilder SELECT Tests (ORM Style)
// =============================================================================

#[serial_test::serial]
#[tokio::test]
async fn test_orm_select_all() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    setup_test_db(pool).await?;

    // ORM: Select all users
    let qb = QueryBuilder::new("orm_users")?;
    let (sql, _params) = qb.build();

    let rows = sqlx::query(&sql).fetch_all(pool).await?;
    assert_eq!(rows.len(), 4);

    teardown_test_db(pool).await?;
    Ok(())
}

#[serial_test::serial]
#[tokio::test]
async fn test_orm_select_specific_columns() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    setup_test_db(pool).await?;

    // ORM: Select only name and email
    let qb =
        QueryBuilder::new("orm_users")?.select(vec!["name".to_string(), "email".to_string()])?;
    let (sql, _) = qb.build();

    assert!(sql.contains("\"name\""));
    assert!(sql.contains("\"email\""));
    assert!(!sql.contains("SELECT *"));

    teardown_test_db(pool).await?;
    Ok(())
}

#[serial_test::serial]
#[tokio::test]
async fn test_orm_where_conditions() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    setup_test_db(pool).await?;

    // ORM: Find active users over 25
    let qb = QueryBuilder::new("orm_users")?
        .where_clause("active", Operator::Eq, ExtractedValue::Bool(true))?
        .where_clause("age", Operator::Gt, ExtractedValue::Int(25))?;
    let (sql, params) = qb.build();

    // Execute with parameters
    let mut query = sqlx::query(&sql);
    for param in &params {
        query = bind_extracted_value(query, param.clone());
    }
    let rows = query.fetch_all(pool).await?;

    assert_eq!(rows.len(), 2); // Alice (30) and Diana (28)

    teardown_test_db(pool).await?;
    Ok(())
}

#[serial_test::serial]
#[tokio::test]
async fn test_orm_where_null() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    setup_test_db(pool).await?;

    // Add a user with NULL age
    sqlx::query("INSERT INTO orm_users (name, email, age) VALUES ('Eve', 'eve@example.com', NULL)")
        .execute(pool)
        .await?;

    // ORM: Find users with NULL age
    let qb = QueryBuilder::new("orm_users")?.where_null("age")?;
    let (sql, _) = qb.build();

    let rows = sqlx::query(&sql).fetch_all(pool).await?;
    assert_eq!(rows.len(), 1);

    teardown_test_db(pool).await?;
    Ok(())
}

#[serial_test::serial]
#[tokio::test]
async fn test_orm_order_and_limit() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    setup_test_db(pool).await?;

    // ORM: Get top 2 oldest users
    let qb = QueryBuilder::new("orm_users")?
        .order_by("age", OrderDirection::Desc)?
        .limit(2);
    let (sql, params) = qb.build();

    let mut query = sqlx::query(&sql);
    for param in &params {
        query = bind_extracted_value(query, param.clone());
    }
    let rows = query.fetch_all(pool).await?;
    assert_eq!(rows.len(), 2);

    teardown_test_db(pool).await?;
    Ok(())
}

#[serial_test::serial]
#[tokio::test]
async fn test_orm_distinct() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    setup_test_db(pool).await?;

    // ORM: Get distinct categories
    let qb = QueryBuilder::new("orm_products")?
        .select(vec!["category".to_string()])?
        .distinct();
    let (sql, _) = qb.build();

    assert!(sql.contains("DISTINCT"));
    let rows = sqlx::query(&sql).fetch_all(pool).await?;
    assert_eq!(rows.len(), 2); // Electronics, Books

    teardown_test_db(pool).await?;
    Ok(())
}

// =============================================================================
// QueryBuilder JOIN Tests (ORM Style)
// =============================================================================

#[serial_test::serial]
#[tokio::test]
async fn test_orm_inner_join() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    setup_test_db(pool).await?;

    // ORM: Get orders with user names
    let condition = JoinCondition::new("user_id", "orm_users", "id")?;
    let qb = QueryBuilder::new("orm_orders")?
        .select_raw(vec![
            "orm_orders.id".to_string(),
            "orm_users.name".to_string(),
            "orm_orders.total".to_string(),
        ])
        .inner_join("orm_users", None, condition)?;
    let (sql, _) = qb.build();

    let rows = sqlx::query(&sql).fetch_all(pool).await?;
    assert_eq!(rows.len(), 6); // All orders have users

    teardown_test_db(pool).await?;
    Ok(())
}

#[serial_test::serial]
#[tokio::test]
async fn test_orm_left_join() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    setup_test_db(pool).await?;

    // Add a user with no orders
    sqlx::query(
        "INSERT INTO orm_users (name, email, age) VALUES ('Frank', 'frank@example.com', 40)",
    )
    .execute(pool)
    .await?;

    // ORM: Get all users with their orders (including those without orders)
    let condition = JoinCondition::new("id", "orm_orders", "user_id")?;
    let qb = QueryBuilder::new("orm_users")?
        .select_raw(vec![
            "orm_users.name".to_string(),
            "orm_orders.id AS order_id".to_string(),
        ])
        .left_join("orm_orders", None, condition)?;
    let (sql, _) = qb.build();

    let rows = sqlx::query(&sql).fetch_all(pool).await?;
    assert!(rows.len() >= 5); // All users, even Frank with no orders

    teardown_test_db(pool).await?;
    Ok(())
}

#[serial_test::serial]
#[tokio::test]
async fn test_orm_multiple_joins() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    setup_test_db(pool).await?;

    // ORM: Get orders with user and product info
    let user_cond = JoinCondition::new("user_id", "orm_users", "id")?;
    let prod_cond = JoinCondition::new("product_id", "orm_products", "id")?;

    let qb = QueryBuilder::new("orm_orders")?
        .select_raw(vec![
            "orm_orders.id".to_string(),
            "orm_users.name AS user_name".to_string(),
            "orm_products.name AS product_name".to_string(),
            "orm_orders.total".to_string(),
        ])
        .inner_join("orm_users", None, user_cond)?
        .inner_join("orm_products", None, prod_cond)?;
    let (sql, _) = qb.build();

    let rows = sqlx::query(&sql).fetch_all(pool).await?;
    assert_eq!(rows.len(), 6);

    teardown_test_db(pool).await?;
    Ok(())
}

// =============================================================================
// QueryBuilder Aggregate Tests (ORM Style)
// =============================================================================

#[serial_test::serial]
#[tokio::test]
async fn test_orm_count() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    setup_test_db(pool).await?;

    // ORM: Count all users
    let qb = QueryBuilder::new("orm_users")?.count_agg(Some("total"))?;
    let (sql, _) = qb.build();

    let row = sqlx::query(&sql).fetch_one(pool).await?;
    let count: i64 = sqlx::Row::get(&row, "total");
    assert_eq!(count, 4);

    teardown_test_db(pool).await?;
    Ok(())
}

#[serial_test::serial]
#[tokio::test]
async fn test_orm_sum_avg() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    setup_test_db(pool).await?;

    // ORM: Calculate total and average order amounts
    let qb = QueryBuilder::new("orm_orders")?
        .sum("total", Some("total_sales"))?
        .avg("total", Some("avg_order"))?;
    let (sql, _) = qb.build();

    let row = sqlx::query(&sql).fetch_one(pool).await?;
    let _total: rust_decimal::Decimal = sqlx::Row::get(&row, "total_sales");
    let _avg: rust_decimal::Decimal = sqlx::Row::get(&row, "avg_order");

    teardown_test_db(pool).await?;
    Ok(())
}

#[serial_test::serial]
#[tokio::test]
async fn test_orm_group_by() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    setup_test_db(pool).await?;

    // ORM: Count orders per status
    let qb = QueryBuilder::new("orm_orders")?
        .select(vec!["status".to_string()])?
        .count_agg(Some("count"))?
        .group_by(&["status"])?
        .order_by("count", OrderDirection::Desc)?;
    let (sql, _) = qb.build();

    let rows = sqlx::query(&sql).fetch_all(pool).await?;
    assert_eq!(rows.len(), 3); // completed, pending, shipped

    teardown_test_db(pool).await?;
    Ok(())
}

#[serial_test::serial]
#[tokio::test]
async fn test_orm_having() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    setup_test_db(pool).await?;

    // ORM: Find users with more than 1 order
    let qb = QueryBuilder::new("orm_orders")?
        .select(vec!["user_id".to_string()])?
        .count_agg(Some("order_count"))?
        .group_by(&["user_id"])?
        .having_count(Operator::Gt, ExtractedValue::Int(1))?;
    let (sql, params) = qb.build();

    let mut query = sqlx::query(&sql);
    for param in &params {
        query = bind_extracted_value(query, param.clone());
    }
    let rows = query.fetch_all(pool).await?;

    assert_eq!(rows.len(), 2); // Alice and Diana have > 1 order

    teardown_test_db(pool).await?;
    Ok(())
}

// =============================================================================
// QueryBuilder Window Function Tests (ORM Style)
// =============================================================================

#[serial_test::serial]
#[tokio::test]
async fn test_orm_row_number() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    setup_test_db(pool).await?;

    // ORM: Rank orders by total within each status
    let spec = WindowSpec::new()
        .partition_by(&["status"])
        .order_by("total", OrderDirection::Desc);

    let qb = QueryBuilder::new("orm_orders")?.row_number(spec, "rank")?;
    let (sql, _) = qb.build();

    assert!(sql.contains("ROW_NUMBER()"));
    assert!(sql.contains("PARTITION BY"));

    let rows = sqlx::query(&sql).fetch_all(pool).await?;
    assert_eq!(rows.len(), 6);

    teardown_test_db(pool).await?;
    Ok(())
}

#[serial_test::serial]
#[tokio::test]
async fn test_orm_rank_dense_rank() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    setup_test_db(pool).await?;

    let spec = WindowSpec::new().order_by("price", OrderDirection::Desc);

    let qb = QueryBuilder::new("orm_products")?
        .rank(spec.clone(), "price_rank")?
        .dense_rank(spec, "dense_price_rank")?;
    let (sql, _) = qb.build();

    assert!(sql.contains("RANK()"));
    assert!(sql.contains("DENSE_RANK()"));

    let rows = sqlx::query(&sql).fetch_all(pool).await?;
    assert_eq!(rows.len(), 4);

    teardown_test_db(pool).await?;
    Ok(())
}

// =============================================================================
// Transaction Tests (ORM Style)
// =============================================================================

#[serial_test::serial]
#[tokio::test]
async fn test_orm_transaction_commit() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    setup_test_db(pool).await?;

    // ORM: Transaction commit
    let mut tx = Transaction::begin(&conn, IsolationLevel::ReadCommitted).await?;

    sqlx::query("INSERT INTO orm_users (name, email, age) VALUES ('TxUser', 'tx@example.com', 25)")
        .execute(tx.as_mut_transaction().as_mut())
        .await?;

    tx.commit().await?;

    // Verify committed
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM orm_users WHERE name = 'TxUser'")
        .fetch_one(pool)
        .await?;
    assert_eq!(count.0, 1);

    teardown_test_db(pool).await?;
    Ok(())
}

#[serial_test::serial]
#[tokio::test]
async fn test_orm_transaction_rollback() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    setup_test_db(pool).await?;

    let initial_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM orm_users")
        .fetch_one(pool)
        .await?;

    // ORM: Transaction rollback
    let mut tx = Transaction::begin(&conn, IsolationLevel::ReadCommitted).await?;

    sqlx::query(
        "INSERT INTO orm_users (name, email, age) VALUES ('RollbackUser', 'rb@example.com', 25)",
    )
    .execute(tx.as_mut_transaction().as_mut())
    .await?;

    tx.rollback().await?;

    // Verify rolled back
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM orm_users")
        .fetch_one(pool)
        .await?;
    assert_eq!(count.0, initial_count.0);

    teardown_test_db(pool).await?;
    Ok(())
}

#[serial_test::serial]
#[tokio::test]
async fn test_orm_transaction_savepoint() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    setup_test_db(pool).await?;

    // ORM: Savepoint usage
    let mut tx = Transaction::begin(&conn, IsolationLevel::ReadCommitted).await?;

    // Insert user 1
    sqlx::query("INSERT INTO orm_users (name, email, age) VALUES ('SP1', 'sp1@example.com', 25)")
        .execute(tx.as_mut_transaction().as_mut())
        .await?;

    // Create savepoint
    tx.savepoint("sp1").await?;

    // Insert user 2
    sqlx::query("INSERT INTO orm_users (name, email, age) VALUES ('SP2', 'sp2@example.com', 30)")
        .execute(tx.as_mut_transaction().as_mut())
        .await?;

    // Rollback to savepoint (undoes user 2)
    tx.rollback_to("sp1").await?;

    // Commit (only user 1 should exist)
    tx.commit().await?;

    // Verify
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM orm_users WHERE name LIKE 'SP%'")
        .fetch_one(pool)
        .await?;
    assert_eq!(count.0, 1);

    teardown_test_db(pool).await?;
    Ok(())
}

#[serial_test::serial]
#[tokio::test]
async fn test_orm_transaction_isolation_levels() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    setup_test_db(pool).await?;

    // Test different isolation levels
    for level in [
        IsolationLevel::ReadCommitted,
        IsolationLevel::RepeatableRead,
        IsolationLevel::Serializable,
    ] {
        let tx = Transaction::begin(&conn, level).await?;
        tx.rollback().await?;
    }

    teardown_test_db(pool).await?;
    Ok(())
}

// =============================================================================
// Bulk Operations Tests (ORM Style)
// =============================================================================

#[serial_test::serial]
#[tokio::test]
async fn test_orm_bulk_insert() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    setup_test_db(pool).await?;

    let executor = BulkExecutor::new(&conn, BulkConfig::new().batch_size(10));

    // ORM: Bulk insert products
    let products: Vec<HashMap<String, ExtractedValue>> = (0..50)
        .map(|i| {
            let mut row = HashMap::new();
            row.insert(
                "name".to_string(),
                ExtractedValue::String(format!("BulkProduct{}", i)),
            );
            row.insert("price".to_string(), ExtractedValue::Double(10.0 + i as f64));
            row.insert(
                "category".to_string(),
                ExtractedValue::String("Bulk".to_string()),
            );
            row.insert("stock".to_string(), ExtractedValue::Int(i * 10));
            row
        })
        .collect();

    let result = executor.insert_parallel("orm_products", &products).await?;
    assert_eq!(result.success_count, 50);

    teardown_test_db(pool).await?;
    Ok(())
}

#[serial_test::serial]
#[tokio::test]
async fn test_orm_bulk_update() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    setup_test_db(pool).await?;

    let executor = BulkExecutor::new(&conn, BulkConfig::default());

    // ORM: Bulk update - increase all product prices by setting stock
    let updates: Vec<HashMap<String, ExtractedValue>> = (1..=4)
        .map(|i| {
            let mut row = HashMap::new();
            row.insert("id".to_string(), ExtractedValue::Int(i));
            row.insert("stock".to_string(), ExtractedValue::Int(999));
            row
        })
        .collect();

    let result = executor.update_parallel("orm_products", &updates).await?;
    assert_eq!(result.success_count, 4);

    // Verify
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM orm_products WHERE stock = 999")
        .fetch_one(pool)
        .await?;
    assert_eq!(count.0, 4);

    teardown_test_db(pool).await?;
    Ok(())
}

#[serial_test::serial]
#[tokio::test]
async fn test_orm_bulk_delete() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    setup_test_db(pool).await?;

    // Add some deletable products
    for i in 100..110 {
        sqlx::query(&format!(
            "INSERT INTO orm_products (id, name, price, category) VALUES ({}, 'Del{}', 1.00, 'Delete')",
            i, i
        ))
        .execute(pool).await?;
    }

    let executor = BulkExecutor::new(&conn, BulkConfig::default());

    // ORM: Bulk delete
    let ids: Vec<i64> = (100..110).collect();
    let result = executor.delete_parallel("orm_products", &ids).await?;
    assert_eq!(result.success_count, 10);

    // Verify
    let count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM orm_products WHERE category = 'Delete'")
            .fetch_one(pool)
            .await?;
    assert_eq!(count.0, 0);

    teardown_test_db(pool).await?;
    Ok(())
}

// =============================================================================
// Complex ORM Queries (Real-World Scenarios)
// =============================================================================

#[serial_test::serial]
#[tokio::test]
async fn test_orm_complex_report_query() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    setup_test_db(pool).await?;

    // ORM: Complex report - Sales by user with product details
    let user_cond = JoinCondition::new("user_id", "orm_users", "id")?;
    let prod_cond = JoinCondition::new("product_id", "orm_products", "id")?;

    let qb = QueryBuilder::new("orm_orders")?
        .select_raw(vec![
            "orm_users.name AS customer".to_string(),
            "orm_products.category".to_string(),
        ])
        .inner_join("orm_users", None, user_cond)?
        .inner_join("orm_products", None, prod_cond)?
        .count_agg(Some("order_count"))?
        .sum("orm_orders.total", Some("total_spent"))?
        .group_by(&["orm_users.name", "orm_products.category"])?
        .order_by("total_spent", OrderDirection::Desc)?;

    let (sql, _) = qb.build();
    let rows = sqlx::query(&sql).fetch_all(pool).await?;
    assert!(!rows.is_empty());

    teardown_test_db(pool).await?;
    Ok(())
}

#[serial_test::serial]
#[tokio::test]
async fn test_orm_subquery_filter() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    setup_test_db(pool).await?;

    // ORM: Find users who have placed orders (using subquery)
    let subquery = QueryBuilder::new("orm_orders")?.select(vec!["user_id".to_string()])?;

    let qb = QueryBuilder::new("orm_users")?.where_in_subquery("id", subquery)?;

    let (sql, _) = qb.build();
    let rows = sqlx::query(&sql).fetch_all(pool).await?;
    assert_eq!(rows.len(), 4); // All 4 users have orders

    teardown_test_db(pool).await?;
    Ok(())
}

#[serial_test::serial]
#[tokio::test]
async fn test_orm_pagination() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    setup_test_db(pool).await?;

    // ORM: Paginated product listing
    let page_size = 2;

    // Page 1
    let qb = QueryBuilder::new("orm_products")?
        .order_by("id", OrderDirection::Asc)?
        .limit(page_size)
        .offset(0);
    let (sql, params) = qb.build();
    let mut query = sqlx::query(&sql);
    for param in &params {
        query = bind_extracted_value(query, param.clone());
    }
    let page1 = query.fetch_all(pool).await?;
    assert_eq!(page1.len(), 2);

    // Page 2
    let qb = QueryBuilder::new("orm_products")?
        .order_by("id", OrderDirection::Asc)?
        .limit(page_size)
        .offset(page_size);
    let (sql, params) = qb.build();
    let mut query = sqlx::query(&sql);
    for param in &params {
        query = bind_extracted_value(query, param.clone());
    }
    let page2 = query.fetch_all(pool).await?;
    assert_eq!(page2.len(), 2);

    teardown_test_db(pool).await?;
    Ok(())
}

#[serial_test::serial]
#[tokio::test]
async fn test_orm_search_like() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    setup_test_db(pool).await?;

    // ORM: Search products by name
    let qb = QueryBuilder::new("orm_products")?.where_clause(
        "name",
        Operator::ILike,
        ExtractedValue::String("%phone%".to_string()),
    )?;

    let (sql, params) = qb.build();
    let mut query = sqlx::query(&sql);
    for param in &params {
        query = bind_extracted_value(query, param.clone());
    }
    let rows = query.fetch_all(pool).await?;
    assert_eq!(rows.len(), 2); // Phone, Headphones

    teardown_test_db(pool).await?;
    Ok(())
}

// =============================================================================
// Helper Functions
// =============================================================================

fn bind_extracted_value<'q>(
    query: sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments>,
    value: ExtractedValue,
) -> sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments> {
    match value {
        ExtractedValue::Null => query.bind(Option::<String>::None),
        ExtractedValue::Bool(b) => query.bind(b),
        ExtractedValue::SmallInt(i) => query.bind(i),
        ExtractedValue::Int(i) => query.bind(i),
        ExtractedValue::BigInt(i) => query.bind(i),
        ExtractedValue::Float(f) => query.bind(f),
        ExtractedValue::Double(f) => query.bind(f),
        ExtractedValue::String(s) => query.bind(s),
        ExtractedValue::Bytes(b) => query.bind(b),
        ExtractedValue::Json(j) => query.bind(j),
        _ => query.bind(Option::<String>::None),
    }
}
