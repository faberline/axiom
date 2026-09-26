//! Integration tests for Row CRUD operations.
//!
//! These tests require a PostgreSQL database to be running.
//! Set DATABASE_URL environment variable or skip with SKIP_INTEGRATION=true

use pg::{
    Connection, ExtractedValue, Operator, OrderDirection, PoolConfig, QueryBuilder, Row,
};
use qc::{expect, AssertionError};

#[tokio::test]
#[ignore] // Only run with --ignored flag when database is available
async fn test_insert_and_find_by_id() -> Result<(), Box<dyn std::error::Error>> {
    // This test requires a real PostgreSQL connection
    let uri = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://localhost/test_db".to_string());

    let conn = Connection::new(&uri, PoolConfig::default()).await.unwrap();
    let pool = conn.pool();

    // Create test table
    sqlx::query("DROP TABLE IF EXISTS test_users CASCADE")
        .execute(pool)
        .await
        .unwrap();

    sqlx::query(
        "CREATE TABLE test_users (
            id BIGSERIAL PRIMARY KEY,
            name TEXT NOT NULL,
            age INTEGER NOT NULL,
            active BOOLEAN DEFAULT true
        )",
    )
    .execute(pool)
    .await
    .unwrap();

    // Insert a row
    let values = vec![
        (
            "name".to_string(),
            ExtractedValue::String("Alice".to_string()),
        ),
        ("age".to_string(), ExtractedValue::Int(30)),
        ("active".to_string(), ExtractedValue::Bool(true)),
    ];

    let inserted_row = Row::insert(pool, "test_users", &values).await.unwrap();

    // Verify inserted row has id
    let id_value = inserted_row.get("id").unwrap();
    let id = match id_value {
        ExtractedValue::BigInt(i) => *i,
        _ => panic!("Expected BigInt for id"),
    };

    expect(id).to_be_greater_than(&0)?;

    // Find by ID
    let found_row = Row::find_by_id(pool, "test_users", id)
        .await
        .unwrap()
        .expect("Row should exist");

    // Verify values
    expect(matches!(found_row.get("name").unwrap(), ExtractedValue::String(s) if s == "Alice"))
        .to_be_true()?;
    expect(matches!(
        found_row.get("age").unwrap(),
        ExtractedValue::Int(30)
    ))
    .to_be_true()?;
    expect(matches!(
        found_row.get("active").unwrap(),
        ExtractedValue::Bool(true)
    ))
    .to_be_true()?;

    // Clean up
    sqlx::query("DROP TABLE test_users CASCADE")
        .execute(pool)
        .await
        .unwrap();

    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_find_many_with_filters() -> Result<(), Box<dyn std::error::Error>> {
    let uri = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://localhost/test_db".to_string());

    let conn = Connection::new(&uri, PoolConfig::default()).await.unwrap();
    let pool = conn.pool();

    // Create test table
    sqlx::query("DROP TABLE IF EXISTS test_products CASCADE")
        .execute(pool)
        .await
        .unwrap();

    sqlx::query(
        "CREATE TABLE test_products (
            id BIGSERIAL PRIMARY KEY,
            name TEXT NOT NULL,
            price INTEGER NOT NULL
        )",
    )
    .execute(pool)
    .await
    .unwrap();

    // Insert test data
    let products = vec![
        ("Product A", 100),
        ("Product B", 200),
        ("Product C", 150),
        ("Product D", 250),
    ];

    for (name, price) in products {
        let values = vec![
            ("name".to_string(), ExtractedValue::String(name.to_string())),
            ("price".to_string(), ExtractedValue::Int(price)),
        ];
        Row::insert(pool, "test_products", &values).await.unwrap();
    }

    // Find all products with price >= 150
    let query = QueryBuilder::new("test_products")
        .unwrap()
        .where_clause("price", Operator::Gte, ExtractedValue::Int(150))
        .unwrap()
        .order_by("price", OrderDirection::Asc)
        .unwrap();

    let results = Row::find_many(pool, "test_products", Some(&query))
        .await
        .unwrap();

    expect(results.len()).to_equal(&3)?; // Products C, B, D

    // Verify first result is Product C (150)
    expect(matches!(
        results[0].get("name").unwrap(),
        ExtractedValue::String(s) if s == "Product C"
    ))
    .to_be_true()?;

    // Clean up
    sqlx::query("DROP TABLE test_products CASCADE")
        .execute(pool)
        .await
        .unwrap();

    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_update_and_delete() -> Result<(), Box<dyn std::error::Error>> {
    let uri = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://localhost/test_db".to_string());

    let conn = Connection::new(&uri, PoolConfig::default()).await.unwrap();
    let pool = conn.pool();

    // Create test table
    sqlx::query("DROP TABLE IF EXISTS test_items CASCADE")
        .execute(pool)
        .await
        .unwrap();

    sqlx::query(
        "CREATE TABLE test_items (
            id BIGSERIAL PRIMARY KEY,
            title TEXT NOT NULL,
            quantity INTEGER NOT NULL
        )",
    )
    .execute(pool)
    .await
    .unwrap();

    // Insert item
    let values = vec![
        (
            "title".to_string(),
            ExtractedValue::String("Item 1".to_string()),
        ),
        ("quantity".to_string(), ExtractedValue::Int(10)),
    ];
    let row = Row::insert(pool, "test_items", &values).await.unwrap();
    let id = match row.get("id").unwrap() {
        ExtractedValue::BigInt(i) => *i,
        _ => panic!("Expected BigInt"),
    };

    // Update item
    let updates = vec![
        (
            "title".to_string(),
            ExtractedValue::String("Updated Item".to_string()),
        ),
        ("quantity".to_string(), ExtractedValue::Int(20)),
    ];
    let updated = Row::update(pool, "test_items", id, &updates).await.unwrap();
    expect(updated).to_be_true()?;

    // Verify update
    let found = Row::find_by_id(pool, "test_items", id)
        .await
        .unwrap()
        .expect("Row should exist");
    expect(matches!(
        found.get("title").unwrap(),
        ExtractedValue::String(s) if s == "Updated Item"
    ))
    .to_be_true()?;
    expect(matches!(
        found.get("quantity").unwrap(),
        ExtractedValue::Int(20)
    ))
    .to_be_true()?;

    // Delete item
    let deleted = Row::delete(pool, "test_items", id).await.unwrap();
    expect(deleted).to_be_true()?;

    // Verify deletion
    let not_found = Row::find_by_id(pool, "test_items", id).await.unwrap();
    expect(not_found.is_none()).to_be_true()?;

    // Clean up
    sqlx::query("DROP TABLE test_items CASCADE")
        .execute(pool)
        .await
        .unwrap();

    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_count() -> Result<(), Box<dyn std::error::Error>> {
    let uri = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://localhost/test_db".to_string());

    let conn = Connection::new(&uri, PoolConfig::default()).await.unwrap();
    let pool = conn.pool();

    // Create test table
    sqlx::query("DROP TABLE IF EXISTS test_counts CASCADE")
        .execute(pool)
        .await
        .unwrap();

    sqlx::query(
        "CREATE TABLE test_counts (
            id BIGSERIAL PRIMARY KEY,
            status TEXT NOT NULL
        )",
    )
    .execute(pool)
    .await
    .unwrap();

    // Insert test data
    for status in &["active", "active", "inactive", "active", "pending"] {
        let values = vec![(
            "status".to_string(),
            ExtractedValue::String(status.to_string()),
        )];
        Row::insert(pool, "test_counts", &values).await.unwrap();
    }

    // Count all rows
    let total = Row::count(pool, "test_counts", None).await.unwrap();
    expect(total).to_equal(&5)?;

    // Count active rows
    let query = QueryBuilder::new("test_counts")
        .unwrap()
        .where_clause(
            "status",
            Operator::Eq,
            ExtractedValue::String("active".to_string()),
        )
        .unwrap();
    let active_count = Row::count(pool, "test_counts", Some(&query)).await.unwrap();
    expect(active_count).to_equal(&3)?;

    // Clean up
    sqlx::query("DROP TABLE test_counts CASCADE")
        .execute(pool)
        .await
        .unwrap();

    Ok(())
}

#[test]
fn test_row_to_json() -> Result<(), AssertionError> {
    use std::collections::HashMap;

    let mut columns = HashMap::new();
    columns.insert("id".to_string(), ExtractedValue::BigInt(42));
    columns.insert(
        "name".to_string(),
        ExtractedValue::String("Test".to_string()),
    );
    columns.insert("active".to_string(), ExtractedValue::Bool(true));
    columns.insert("price".to_string(), ExtractedValue::Double(99.99));

    let row = Row::new(columns);
    let json = row.to_json().unwrap();

    expect(json.is_object()).to_be_true()?;
    let obj = json.as_object().unwrap();
    expect(obj.get("id").unwrap().as_i64().unwrap()).to_equal(&42)?;
    expect(obj.get("name").unwrap().as_str().unwrap()).to_equal(&"Test")?;
    expect(obj.get("active").unwrap().as_bool().unwrap()).to_be_true()?;
    expect((obj.get("price").unwrap().as_f64().unwrap() - 99.99).abs() < 0.01).to_be_true()?;

    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_insert_many() -> Result<(), Box<dyn std::error::Error>> {
    use std::collections::HashMap;

    let uri = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://localhost/test_db".to_string());

    let conn = Connection::new(&uri, PoolConfig::default()).await.unwrap();
    let pool = conn.pool();

    // Create test table
    sqlx::query("DROP TABLE IF EXISTS test_batch_inserts CASCADE")
        .execute(pool)
        .await
        .unwrap();

    sqlx::query(
        "CREATE TABLE test_batch_inserts (
            id BIGSERIAL PRIMARY KEY,
            name TEXT NOT NULL,
            age INTEGER NOT NULL,
            score DOUBLE PRECISION NOT NULL
        )",
    )
    .execute(pool)
    .await
    .unwrap();

    // Prepare multiple rows to insert
    let mut row1 = HashMap::new();
    row1.insert(
        "name".to_string(),
        ExtractedValue::String("Alice".to_string()),
    );
    row1.insert("age".to_string(), ExtractedValue::Int(30));
    row1.insert("score".to_string(), ExtractedValue::Double(95.5));

    let mut row2 = HashMap::new();
    row2.insert(
        "name".to_string(),
        ExtractedValue::String("Bob".to_string()),
    );
    row2.insert("age".to_string(), ExtractedValue::Int(25));
    row2.insert("score".to_string(), ExtractedValue::Double(87.3));

    let mut row3 = HashMap::new();
    row3.insert(
        "name".to_string(),
        ExtractedValue::String("Charlie".to_string()),
    );
    row3.insert("age".to_string(), ExtractedValue::Int(35));
    row3.insert("score".to_string(), ExtractedValue::Double(92.1));

    // Batch insert
    let rows = Row::insert_many(pool, "test_batch_inserts", &[row1, row2, row3])
        .await
        .unwrap();

    // Verify all rows were inserted
    expect(rows.len()).to_equal(&3)?;

    // Verify each row has an ID and correct values
    for (idx, row) in rows.iter().enumerate() {
        // Check ID is present and positive
        let id = match row.get("id").unwrap() {
            ExtractedValue::BigInt(i) => *i,
            _ => panic!("Expected BigInt for id"),
        };
        expect(id).to_be_greater_than(&0)?;

        // Verify other fields
        match idx {
            0 => {
                expect(
                    matches!(row.get("name").unwrap(), ExtractedValue::String(s) if s == "Alice"),
                )
                .to_be_true()?;
                expect(matches!(row.get("age").unwrap(), ExtractedValue::Int(30))).to_be_true()?;
                expect(matches!(row.get("score").unwrap(), ExtractedValue::Double(s) if (*s - 95.5).abs() < 0.01)).to_be_true()?;
            }
            1 => {
                expect(matches!(row.get("name").unwrap(), ExtractedValue::String(s) if s == "Bob"))
                    .to_be_true()?;
                expect(matches!(row.get("age").unwrap(), ExtractedValue::Int(25))).to_be_true()?;
                expect(matches!(row.get("score").unwrap(), ExtractedValue::Double(s) if (*s - 87.3).abs() < 0.01)).to_be_true()?;
            }
            2 => {
                expect(
                    matches!(row.get("name").unwrap(), ExtractedValue::String(s) if s == "Charlie"),
                )
                .to_be_true()?;
                expect(matches!(row.get("age").unwrap(), ExtractedValue::Int(35))).to_be_true()?;
                expect(matches!(row.get("score").unwrap(), ExtractedValue::Double(s) if (*s - 92.1).abs() < 0.01)).to_be_true()?;
            }
            _ => unreachable!(),
        }
    }

    // Verify count in database
    let count = Row::count(pool, "test_batch_inserts", None).await.unwrap();
    expect(count).to_equal(&3)?;

    // Clean up
    sqlx::query("DROP TABLE test_batch_inserts CASCADE")
        .execute(pool)
        .await
        .unwrap();

    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_insert_many_empty() -> Result<(), Box<dyn std::error::Error>> {
    use std::collections::HashMap;

    let uri = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://localhost/test_db".to_string());

    let conn = Connection::new(&uri, PoolConfig::default()).await.unwrap();
    let pool = conn.pool();

    // Empty insert should return empty vec
    let rows: Vec<HashMap<String, ExtractedValue>> = vec![];
    let result = Row::insert_many(pool, "test_table", &rows).await.unwrap();
    expect(result.len()).to_equal(&0)?;

    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_insert_many_mismatched_columns() -> Result<(), Box<dyn std::error::Error>> {
    use std::collections::HashMap;

    let uri = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://localhost/test_db".to_string());

    let conn = Connection::new(&uri, PoolConfig::default()).await.unwrap();
    let pool = conn.pool();

    // Create test table
    sqlx::query("DROP TABLE IF EXISTS test_mismatch CASCADE")
        .execute(pool)
        .await
        .unwrap();

    sqlx::query(
        "CREATE TABLE test_mismatch (
            id BIGSERIAL PRIMARY KEY,
            name TEXT NOT NULL,
            age INTEGER
        )",
    )
    .execute(pool)
    .await
    .unwrap();

    // First row has name and age
    let mut row1 = HashMap::new();
    row1.insert(
        "name".to_string(),
        ExtractedValue::String("Alice".to_string()),
    );
    row1.insert("age".to_string(), ExtractedValue::Int(30));

    // Second row only has name (missing age)
    let mut row2 = HashMap::new();
    row2.insert(
        "name".to_string(),
        ExtractedValue::String("Bob".to_string()),
    );

    // Should fail due to mismatched columns
    let result = Row::insert_many(pool, "test_mismatch", &[row1, row2]).await;
    expect(result.is_err()).to_be_true()?;

    // Clean up
    sqlx::query("DROP TABLE test_mismatch CASCADE")
        .execute(pool)
        .await
        .unwrap();

    Ok(())
}
