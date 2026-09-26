//! Integration tests for PostgreSQL type handling.
//!
//! Tests ExtractedValue conversions, type roundtrips, and edge cases
//! for all supported PostgreSQL types.
//!
//! Run with: cargo test -p cclab-titan --test test_types

use chrono::{NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};
use pg::{Connection, ExtractedValue, PoolConfig};
use rust_decimal::Decimal;
use serde_json::json;
use sqlx::Row;
use std::str::FromStr;
use uuid::Uuid;

fn get_database_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql://localhost/test_db".to_string())
}

async fn cleanup_table(pool: &sqlx::PgPool, table: &str) -> Result<(), sqlx::Error> {
    sqlx::query(&format!("DROP TABLE IF EXISTS {} CASCADE", table))
        .execute(pool)
        .await?;
    Ok(())
}

// =============================================================================
// ExtractedValue Unit Tests
// =============================================================================

#[test]
fn test_extracted_value_pg_type_names() {
    assert_eq!(ExtractedValue::Null.pg_type_name(), "NULL");
    assert_eq!(ExtractedValue::Bool(true).pg_type_name(), "BOOLEAN");
    assert_eq!(ExtractedValue::SmallInt(1).pg_type_name(), "SMALLINT");
    assert_eq!(ExtractedValue::Int(1).pg_type_name(), "INTEGER");
    assert_eq!(ExtractedValue::BigInt(1).pg_type_name(), "BIGINT");
    assert_eq!(ExtractedValue::Float(1.0).pg_type_name(), "REAL");
    assert_eq!(
        ExtractedValue::Double(1.0).pg_type_name(),
        "DOUBLE PRECISION"
    );
    assert_eq!(
        ExtractedValue::String("test".to_string()).pg_type_name(),
        "TEXT"
    );
    assert_eq!(ExtractedValue::Bytes(vec![1, 2, 3]).pg_type_name(), "BYTEA");
    assert_eq!(ExtractedValue::Uuid(Uuid::new_v4()).pg_type_name(), "UUID");
    assert_eq!(ExtractedValue::Json(json!({})).pg_type_name(), "JSONB");
    assert_eq!(ExtractedValue::Array(vec![]).pg_type_name(), "ARRAY");
    assert_eq!(
        ExtractedValue::Decimal(Decimal::new(100, 2)).pg_type_name(),
        "NUMERIC"
    );
}

#[test]
fn test_extracted_value_equality() {
    assert_eq!(ExtractedValue::Int(42), ExtractedValue::Int(42));
    assert_ne!(ExtractedValue::Int(42), ExtractedValue::Int(43));
    assert_ne!(ExtractedValue::Int(42), ExtractedValue::BigInt(42));
    assert_eq!(
        ExtractedValue::String("test".to_string()),
        ExtractedValue::String("test".to_string())
    );
    assert_eq!(
        ExtractedValue::Array(vec![ExtractedValue::Int(1), ExtractedValue::Int(2)]),
        ExtractedValue::Array(vec![ExtractedValue::Int(1), ExtractedValue::Int(2)])
    );
}

#[test]
fn test_extracted_value_clone() {
    let original = ExtractedValue::Json(json!({"key": "value"}));
    let cloned = original.clone();
    assert_eq!(original, cloned);
}

// =============================================================================
// Integer Type Tests
// =============================================================================

#[tokio::test]
async fn test_integer_types_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_int_types";

    cleanup_table(pool, table).await?;
    sqlx::query(&format!(
        "CREATE TABLE {} (
            id SERIAL PRIMARY KEY,
            small_val SMALLINT,
            int_val INTEGER,
            big_val BIGINT
        )",
        table
    ))
    .execute(pool)
    .await?;

    // Insert boundary values
    sqlx::query(&format!(
        "INSERT INTO {} (small_val, int_val, big_val) VALUES ($1, $2, $3)",
        table
    ))
    .bind(i16::MAX)
    .bind(i32::MAX)
    .bind(i64::MAX)
    .execute(pool)
    .await?;

    sqlx::query(&format!(
        "INSERT INTO {} (small_val, int_val, big_val) VALUES ($1, $2, $3)",
        table
    ))
    .bind(i16::MIN)
    .bind(i32::MIN)
    .bind(i64::MIN)
    .execute(pool)
    .await?;

    // Verify values
    let rows = sqlx::query(&format!("SELECT * FROM {} ORDER BY id", table))
        .fetch_all(pool)
        .await?;

    assert_eq!(rows.len(), 2);

    let row1 = &rows[0];
    assert_eq!(row1.get::<i16, _>("small_val"), i16::MAX);
    assert_eq!(row1.get::<i32, _>("int_val"), i32::MAX);
    assert_eq!(row1.get::<i64, _>("big_val"), i64::MAX);

    let row2 = &rows[1];
    assert_eq!(row2.get::<i16, _>("small_val"), i16::MIN);
    assert_eq!(row2.get::<i32, _>("int_val"), i32::MIN);
    assert_eq!(row2.get::<i64, _>("big_val"), i64::MIN);

    cleanup_table(pool, table).await?;
    Ok(())
}

#[tokio::test]
async fn test_integer_null_handling() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_int_null";

    cleanup_table(pool, table).await?;
    sqlx::query(&format!(
        "CREATE TABLE {} (id SERIAL PRIMARY KEY, nullable_int INTEGER)",
        table
    ))
    .execute(pool)
    .await?;

    sqlx::query(&format!(
        "INSERT INTO {} (nullable_int) VALUES (NULL)",
        table
    ))
    .execute(pool)
    .await?;

    let row = sqlx::query(&format!("SELECT nullable_int FROM {}", table))
        .fetch_one(pool)
        .await?;

    let value: Option<i32> = row.get("nullable_int");
    assert!(value.is_none());

    cleanup_table(pool, table).await?;
    Ok(())
}

// =============================================================================
// Float Type Tests
// =============================================================================

#[tokio::test]
async fn test_float_types_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_float_types";

    cleanup_table(pool, table).await?;
    sqlx::query(&format!(
        "CREATE TABLE {} (
            id SERIAL PRIMARY KEY,
            real_val REAL,
            double_val DOUBLE PRECISION
        )",
        table
    ))
    .execute(pool)
    .await?;

    // Test various float values
    let test_values: Vec<(f32, f64)> = vec![
        (0.0, 0.0),
        (1.5, 1.5),
        (-1.5, -1.5),
        (f32::MIN, f64::MIN),
        (f32::MAX, f64::MAX),
    ];

    for (real_v, double_v) in &test_values {
        sqlx::query(&format!(
            "INSERT INTO {} (real_val, double_val) VALUES ($1, $2)",
            table
        ))
        .bind(*real_v)
        .bind(*double_v)
        .execute(pool)
        .await?;
    }

    let rows = sqlx::query(&format!("SELECT * FROM {} ORDER BY id", table))
        .fetch_all(pool)
        .await?;

    assert_eq!(rows.len(), test_values.len());

    cleanup_table(pool, table).await?;
    Ok(())
}

#[tokio::test]
async fn test_float_special_values() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_float_special";

    cleanup_table(pool, table).await?;
    sqlx::query(&format!(
        "CREATE TABLE {} (id SERIAL PRIMARY KEY, val DOUBLE PRECISION)",
        table
    ))
    .execute(pool)
    .await?;

    // PostgreSQL supports Infinity and NaN
    sqlx::query(&format!("INSERT INTO {} (val) VALUES ('Infinity')", table))
        .execute(pool)
        .await?;
    sqlx::query(&format!("INSERT INTO {} (val) VALUES ('-Infinity')", table))
        .execute(pool)
        .await?;
    sqlx::query(&format!("INSERT INTO {} (val) VALUES ('NaN')", table))
        .execute(pool)
        .await?;

    let rows = sqlx::query(&format!("SELECT val FROM {} ORDER BY id", table))
        .fetch_all(pool)
        .await?;

    assert_eq!(rows.len(), 3);
    let v1: f64 = rows[0].get("val");
    let v2: f64 = rows[1].get("val");
    let v3: f64 = rows[2].get("val");

    assert!(v1.is_infinite() && v1.is_sign_positive());
    assert!(v2.is_infinite() && v2.is_sign_negative());
    assert!(v3.is_nan());

    cleanup_table(pool, table).await?;
    Ok(())
}

// =============================================================================
// String/Text Type Tests
// =============================================================================

#[tokio::test]
async fn test_string_types_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_string_types";

    cleanup_table(pool, table).await?;
    sqlx::query(&format!(
        "CREATE TABLE {} (
            id SERIAL PRIMARY KEY,
            text_val TEXT,
            varchar_val VARCHAR(100)
        )",
        table
    ))
    .execute(pool)
    .await?;

    let test_strings = vec![
        "",
        "hello",
        "hello world",
        "unicode: \u{1F600}\u{1F389}",
        "special: 'quotes' \"double\" \\backslash",
        "multiline\ntext\nhere",
        "tab\there",
    ];

    for s in &test_strings {
        sqlx::query(&format!(
            "INSERT INTO {} (text_val, varchar_val) VALUES ($1, $1)",
            table
        ))
        .bind(*s)
        .execute(pool)
        .await?;
    }

    let rows = sqlx::query(&format!("SELECT * FROM {} ORDER BY id", table))
        .fetch_all(pool)
        .await?;

    assert_eq!(rows.len(), test_strings.len());
    for (i, row) in rows.iter().enumerate() {
        assert_eq!(row.get::<String, _>("text_val"), test_strings[i]);
        assert_eq!(row.get::<String, _>("varchar_val"), test_strings[i]);
    }

    cleanup_table(pool, table).await?;
    Ok(())
}

#[tokio::test]
async fn test_string_sql_injection_safe() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_sql_injection";

    cleanup_table(pool, table).await?;
    sqlx::query(&format!(
        "CREATE TABLE {} (id SERIAL PRIMARY KEY, val TEXT)",
        table
    ))
    .execute(pool)
    .await?;

    // Attempt SQL injection - should be stored as literal text
    let malicious = "'; DROP TABLE users; --";
    sqlx::query(&format!("INSERT INTO {} (val) VALUES ($1)", table))
        .bind(malicious)
        .execute(pool)
        .await?;

    let row = sqlx::query(&format!("SELECT val FROM {}", table))
        .fetch_one(pool)
        .await?;

    assert_eq!(row.get::<String, _>("val"), malicious);

    cleanup_table(pool, table).await?;
    Ok(())
}

// =============================================================================
// UUID Type Tests
// =============================================================================

#[tokio::test]
async fn test_uuid_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_uuid";

    cleanup_table(pool, table).await?;
    sqlx::query(&format!(
        "CREATE TABLE {} (id UUID PRIMARY KEY, name TEXT)",
        table
    ))
    .execute(pool)
    .await?;

    let test_uuid = Uuid::new_v4();
    sqlx::query(&format!(
        "INSERT INTO {} (id, name) VALUES ($1, 'test')",
        table
    ))
    .bind(test_uuid)
    .execute(pool)
    .await?;

    let row = sqlx::query(&format!("SELECT id FROM {}", table))
        .fetch_one(pool)
        .await?;

    assert_eq!(row.get::<Uuid, _>("id"), test_uuid);

    cleanup_table(pool, table).await?;
    Ok(())
}

#[tokio::test]
async fn test_uuid_nil() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_uuid_nil";

    cleanup_table(pool, table).await?;
    sqlx::query(&format!("CREATE TABLE {} (id UUID PRIMARY KEY)", table))
        .execute(pool)
        .await?;

    let nil_uuid = Uuid::nil();
    sqlx::query(&format!("INSERT INTO {} (id) VALUES ($1)", table))
        .bind(nil_uuid)
        .execute(pool)
        .await?;

    let row = sqlx::query(&format!("SELECT id FROM {}", table))
        .fetch_one(pool)
        .await?;

    let retrieved: Uuid = row.get("id");
    assert!(retrieved.is_nil());

    cleanup_table(pool, table).await?;
    Ok(())
}

// =============================================================================
// Date/Time Type Tests
// =============================================================================

#[tokio::test]
async fn test_date_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_date";

    cleanup_table(pool, table).await?;
    sqlx::query(&format!(
        "CREATE TABLE {} (id SERIAL PRIMARY KEY, date_val DATE)",
        table
    ))
    .execute(pool)
    .await?;

    let dates = vec![
        NaiveDate::from_ymd_opt(2000, 1, 1).unwrap(),
        NaiveDate::from_ymd_opt(2023, 12, 31).unwrap(),
        NaiveDate::from_ymd_opt(1970, 1, 1).unwrap(),
    ];

    for d in &dates {
        sqlx::query(&format!("INSERT INTO {} (date_val) VALUES ($1)", table))
            .bind(*d)
            .execute(pool)
            .await?;
    }

    let rows = sqlx::query(&format!("SELECT date_val FROM {} ORDER BY id", table))
        .fetch_all(pool)
        .await?;

    for (i, row) in rows.iter().enumerate() {
        assert_eq!(row.get::<NaiveDate, _>("date_val"), dates[i]);
    }

    cleanup_table(pool, table).await?;
    Ok(())
}

#[tokio::test]
async fn test_time_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_time";

    cleanup_table(pool, table).await?;
    sqlx::query(&format!(
        "CREATE TABLE {} (id SERIAL PRIMARY KEY, time_val TIME)",
        table
    ))
    .execute(pool)
    .await?;

    let times = vec![
        NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
        NaiveTime::from_hms_opt(12, 30, 45).unwrap(),
        NaiveTime::from_hms_opt(23, 59, 59).unwrap(),
    ];

    for t in &times {
        sqlx::query(&format!("INSERT INTO {} (time_val) VALUES ($1)", table))
            .bind(*t)
            .execute(pool)
            .await?;
    }

    let rows = sqlx::query(&format!("SELECT time_val FROM {} ORDER BY id", table))
        .fetch_all(pool)
        .await?;

    for (i, row) in rows.iter().enumerate() {
        assert_eq!(row.get::<NaiveTime, _>("time_val"), times[i]);
    }

    cleanup_table(pool, table).await?;
    Ok(())
}

#[tokio::test]
async fn test_timestamp_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_timestamp";

    cleanup_table(pool, table).await?;
    sqlx::query(&format!(
        "CREATE TABLE {} (
            id SERIAL PRIMARY KEY,
            ts TIMESTAMP,
            tstz TIMESTAMPTZ
        )",
        table
    ))
    .execute(pool)
    .await?;

    let naive_ts = NaiveDateTime::from_str("2023-06-15T10:30:00").unwrap();
    let tz_ts = Utc.with_ymd_and_hms(2023, 6, 15, 10, 30, 0).unwrap();

    sqlx::query(&format!("INSERT INTO {} (ts, tstz) VALUES ($1, $2)", table))
        .bind(naive_ts)
        .bind(tz_ts)
        .execute(pool)
        .await?;

    let row = sqlx::query(&format!("SELECT ts, tstz FROM {}", table))
        .fetch_one(pool)
        .await?;

    assert_eq!(row.get::<NaiveDateTime, _>("ts"), naive_ts);

    cleanup_table(pool, table).await?;
    Ok(())
}

// =============================================================================
// Decimal/Numeric Type Tests
// =============================================================================

#[tokio::test]
async fn test_decimal_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_decimal";

    cleanup_table(pool, table).await?;
    sqlx::query(&format!(
        "CREATE TABLE {} (id SERIAL PRIMARY KEY, val NUMERIC(20, 8))",
        table
    ))
    .execute(pool)
    .await?;

    let decimals = vec![
        Decimal::new(0, 0),
        Decimal::new(12345678, 4),          // 1234.5678
        Decimal::new(-12345678, 4),         // -1234.5678
        Decimal::new(99999999999999999, 8), // Large value
    ];

    for d in &decimals {
        sqlx::query(&format!("INSERT INTO {} (val) VALUES ($1)", table))
            .bind(*d)
            .execute(pool)
            .await?;
    }

    let rows = sqlx::query(&format!("SELECT val FROM {} ORDER BY id", table))
        .fetch_all(pool)
        .await?;

    for (i, row) in rows.iter().enumerate() {
        assert_eq!(row.get::<Decimal, _>("val"), decimals[i]);
    }

    cleanup_table(pool, table).await?;
    Ok(())
}

#[tokio::test]
async fn test_decimal_precision() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_decimal_precision";

    cleanup_table(pool, table).await?;
    sqlx::query(&format!(
        "CREATE TABLE {} (id SERIAL PRIMARY KEY, val NUMERIC(10, 2))",
        table
    ))
    .execute(pool)
    .await?;

    // Test that precision is maintained for money-like values
    let value = Decimal::new(9999999999, 2); // 99999999.99
    sqlx::query(&format!("INSERT INTO {} (val) VALUES ($1)", table))
        .bind(value)
        .execute(pool)
        .await?;

    let row = sqlx::query(&format!("SELECT val FROM {}", table))
        .fetch_one(pool)
        .await?;

    assert_eq!(row.get::<Decimal, _>("val"), value);

    cleanup_table(pool, table).await?;
    Ok(())
}

// =============================================================================
// JSON/JSONB Type Tests
// =============================================================================

#[tokio::test]
async fn test_json_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_json";

    cleanup_table(pool, table).await?;
    sqlx::query(&format!(
        "CREATE TABLE {} (id SERIAL PRIMARY KEY, data JSONB)",
        table
    ))
    .execute(pool)
    .await?;

    let test_jsons = vec![
        json!(null),
        json!(true),
        json!(42),
        json!("string"),
        json!({"key": "value"}),
        json!([1, 2, 3]),
        json!({"nested": {"array": [1, {"deep": true}]}}),
    ];

    for j in &test_jsons {
        sqlx::query(&format!("INSERT INTO {} (data) VALUES ($1)", table))
            .bind(j)
            .execute(pool)
            .await?;
    }

    let rows = sqlx::query(&format!("SELECT data FROM {} ORDER BY id", table))
        .fetch_all(pool)
        .await?;

    for (i, row) in rows.iter().enumerate() {
        assert_eq!(row.get::<serde_json::Value, _>("data"), test_jsons[i]);
    }

    cleanup_table(pool, table).await?;
    Ok(())
}

#[tokio::test]
async fn test_jsonb_operators() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_jsonb_ops";

    cleanup_table(pool, table).await?;
    sqlx::query(&format!(
        "CREATE TABLE {} (id SERIAL PRIMARY KEY, data JSONB)",
        table
    ))
    .execute(pool)
    .await?;

    sqlx::query(&format!(
        "INSERT INTO {} (data) VALUES ($1), ($2), ($3)",
        table
    ))
    .bind(json!({"type": "a", "value": 1}))
    .bind(json!({"type": "b", "value": 2}))
    .bind(json!({"type": "a", "value": 3}))
    .execute(pool)
    .await?;

    // Test @> containment
    let rows = sqlx::query(&format!("SELECT id FROM {} WHERE data @> $1", table))
        .bind(json!({"type": "a"}))
        .fetch_all(pool)
        .await?;

    assert_eq!(rows.len(), 2);

    // Test -> operator
    let row = sqlx::query(&format!(
        "SELECT data->'value' as val FROM {} WHERE id = 1",
        table
    ))
    .fetch_one(pool)
    .await?;

    let val: serde_json::Value = row.get("val");
    assert_eq!(val, json!(1));

    cleanup_table(pool, table).await?;
    Ok(())
}

// =============================================================================
// Array Type Tests
// =============================================================================

#[tokio::test]
async fn test_array_int_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_array_int";

    cleanup_table(pool, table).await?;
    sqlx::query(&format!(
        "CREATE TABLE {} (id SERIAL PRIMARY KEY, vals INTEGER[])",
        table
    ))
    .execute(pool)
    .await?;

    let arrays: Vec<Vec<i32>> = vec![vec![], vec![1], vec![1, 2, 3], vec![-1, 0, 1]];

    for arr in &arrays {
        sqlx::query(&format!("INSERT INTO {} (vals) VALUES ($1)", table))
            .bind(arr)
            .execute(pool)
            .await?;
    }

    let rows = sqlx::query(&format!("SELECT vals FROM {} ORDER BY id", table))
        .fetch_all(pool)
        .await?;

    for (i, row) in rows.iter().enumerate() {
        assert_eq!(row.get::<Vec<i32>, _>("vals"), arrays[i]);
    }

    cleanup_table(pool, table).await?;
    Ok(())
}

#[tokio::test]
async fn test_array_text_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_array_text";

    cleanup_table(pool, table).await?;
    sqlx::query(&format!(
        "CREATE TABLE {} (id SERIAL PRIMARY KEY, tags TEXT[])",
        table
    ))
    .execute(pool)
    .await?;

    let arrays: Vec<Vec<String>> = vec![
        vec![],
        vec!["rust".to_string()],
        vec![
            "rust".to_string(),
            "postgres".to_string(),
            "sqlx".to_string(),
        ],
    ];

    for arr in &arrays {
        sqlx::query(&format!("INSERT INTO {} (tags) VALUES ($1)", table))
            .bind(arr)
            .execute(pool)
            .await?;
    }

    let rows = sqlx::query(&format!("SELECT tags FROM {} ORDER BY id", table))
        .fetch_all(pool)
        .await?;

    for (i, row) in rows.iter().enumerate() {
        assert_eq!(row.get::<Vec<String>, _>("tags"), arrays[i]);
    }

    cleanup_table(pool, table).await?;
    Ok(())
}

#[tokio::test]
async fn test_array_operators() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_array_ops";

    cleanup_table(pool, table).await?;
    sqlx::query(&format!(
        "CREATE TABLE {} (id SERIAL PRIMARY KEY, tags TEXT[])",
        table
    ))
    .execute(pool)
    .await?;

    sqlx::query(&format!(
        "INSERT INTO {} (tags) VALUES ($1), ($2), ($3)",
        table
    ))
    .bind(vec!["rust", "web"])
    .bind(vec!["python", "ml"])
    .bind(vec!["rust", "ml", "web"])
    .execute(pool)
    .await?;

    // Test @> contains
    let rows = sqlx::query(&format!("SELECT id FROM {} WHERE tags @> $1", table))
        .bind(vec!["rust"])
        .fetch_all(pool)
        .await?;

    assert_eq!(rows.len(), 2);

    // Test && overlaps
    let rows = sqlx::query(&format!("SELECT id FROM {} WHERE tags && $1", table))
        .bind(vec!["ml"])
        .fetch_all(pool)
        .await?;

    assert_eq!(rows.len(), 2);

    // Test = ANY
    let rows = sqlx::query(&format!("SELECT id FROM {} WHERE 'web' = ANY(tags)", table))
        .fetch_all(pool)
        .await?;

    assert_eq!(rows.len(), 2);

    cleanup_table(pool, table).await?;
    Ok(())
}

// =============================================================================
// Binary (BYTEA) Type Tests
// =============================================================================

#[tokio::test]
async fn test_bytea_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_bytea";

    cleanup_table(pool, table).await?;
    sqlx::query(&format!(
        "CREATE TABLE {} (id SERIAL PRIMARY KEY, data BYTEA)",
        table
    ))
    .execute(pool)
    .await?;

    let test_data: Vec<Vec<u8>> = vec![
        vec![],
        vec![0],
        vec![0, 1, 2, 3, 255],
        vec![0xFF; 1000], // 1KB of 0xFF
    ];

    for data in &test_data {
        sqlx::query(&format!("INSERT INTO {} (data) VALUES ($1)", table))
            .bind(data)
            .execute(pool)
            .await?;
    }

    let rows = sqlx::query(&format!("SELECT data FROM {} ORDER BY id", table))
        .fetch_all(pool)
        .await?;

    for (i, row) in rows.iter().enumerate() {
        assert_eq!(row.get::<Vec<u8>, _>("data"), test_data[i]);
    }

    cleanup_table(pool, table).await?;
    Ok(())
}

// =============================================================================
// Boolean Type Tests
// =============================================================================

#[tokio::test]
async fn test_boolean_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_boolean";

    cleanup_table(pool, table).await?;
    sqlx::query(&format!(
        "CREATE TABLE {} (id SERIAL PRIMARY KEY, flag BOOLEAN)",
        table
    ))
    .execute(pool)
    .await?;

    sqlx::query(&format!(
        "INSERT INTO {} (flag) VALUES (TRUE), (FALSE), (NULL)",
        table
    ))
    .execute(pool)
    .await?;

    let rows = sqlx::query(&format!("SELECT flag FROM {} ORDER BY id", table))
        .fetch_all(pool)
        .await?;

    assert_eq!(rows[0].get::<bool, _>("flag"), true);
    assert_eq!(rows[1].get::<bool, _>("flag"), false);
    assert!(rows[2].get::<Option<bool>, _>("flag").is_none());

    cleanup_table(pool, table).await?;
    Ok(())
}
