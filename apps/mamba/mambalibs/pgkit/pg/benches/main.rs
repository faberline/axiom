//! Criterion benchmark suite for cclab-titan
//!
//! Benchmarks key performance characteristics:
//! - Bulk Insert (1k rows) - target: <25ms
//! - Complex Query (join + filter) - target: <20ms
//! - Serialization overhead - target: <5ms for 10k rows
//!
//! Usage:
//!   POSTGRES_URL="postgresql://user:pass@localhost/bench_db" cargo bench
//!
//! Requires a PostgreSQL database to be available.

use pg::{
    Connection, ExtractedValue, JoinCondition, JoinType, Operator, OrderDirection, PoolConfig,
    QueryBuilder, Row,
};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::time::Duration;

/// Setup test database connection
async fn setup_connection() -> Connection {
    let uri = std::env::var("POSTGRES_URL")
        .unwrap_or_else(|_| "postgresql://localhost/bench_db".to_string());

    Connection::new(&uri, PoolConfig::default())
        .await
        .expect("Failed to connect to PostgreSQL. Set POSTGRES_URL env var.")
}

/// Setup test tables for benchmarks
async fn setup_test_tables(conn: &Connection) {
    let pool = conn.pool();

    // Drop existing tables
    sqlx::query("DROP TABLE IF EXISTS orders CASCADE")
        .execute(pool)
        .await
        .ok();
    sqlx::query("DROP TABLE IF EXISTS customers CASCADE")
        .execute(pool)
        .await
        .ok();
    sqlx::query("DROP TABLE IF EXISTS products CASCADE")
        .execute(pool)
        .await
        .ok();

    // Create customers table
    sqlx::query(
        "CREATE TABLE customers (
            id BIGSERIAL PRIMARY KEY,
            name TEXT NOT NULL,
            email TEXT NOT NULL,
            created_at TIMESTAMP DEFAULT NOW()
        )",
    )
    .execute(pool)
    .await
    .unwrap();

    // Create products table
    sqlx::query(
        "CREATE TABLE products (
            id BIGSERIAL PRIMARY KEY,
            name TEXT NOT NULL,
            price INTEGER NOT NULL,
            stock INTEGER NOT NULL
        )",
    )
    .execute(pool)
    .await
    .unwrap();

    // Create orders table with foreign key to customers
    sqlx::query(
        "CREATE TABLE orders (
            id BIGSERIAL PRIMARY KEY,
            customer_id BIGINT NOT NULL REFERENCES customers(id),
            product_id BIGINT NOT NULL REFERENCES products(id),
            quantity INTEGER NOT NULL,
            total_price INTEGER NOT NULL,
            created_at TIMESTAMP DEFAULT NOW()
        )",
    )
    .execute(pool)
    .await
    .unwrap();

    // Pre-populate customers and products for join queries
    for i in 1..=100 {
        let values = vec![
            (
                "name".to_string(),
                ExtractedValue::String(format!("Customer {}", i)),
            ),
            (
                "email".to_string(),
                ExtractedValue::String(format!("customer{}@example.com", i)),
            ),
        ];
        Row::insert(pool, "customers", &values).await.unwrap();
    }

    for i in 1..=50 {
        let values = vec![
            (
                "name".to_string(),
                ExtractedValue::String(format!("Product {}", i)),
            ),
            ("price".to_string(), ExtractedValue::Int(100 * i)),
            ("stock".to_string(), ExtractedValue::Int(1000)),
        ];
        Row::insert(pool, "products", &values).await.unwrap();
    }
}

/// Cleanup test tables
async fn cleanup_test_tables(conn: &Connection) {
    let pool = conn.pool();
    sqlx::query("DROP TABLE IF EXISTS orders CASCADE")
        .execute(pool)
        .await
        .ok();
    sqlx::query("DROP TABLE IF EXISTS customers CASCADE")
        .execute(pool)
        .await
        .ok();
    sqlx::query("DROP TABLE IF EXISTS products CASCADE")
        .execute(pool)
        .await
        .ok();
}

/// Benchmark: Bulk Insert (1k rows)
/// Target: <25ms
fn bench_bulk_insert(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let conn = runtime.block_on(setup_connection());
    runtime.block_on(setup_test_tables(&conn));

    let pool = conn.pool();

    c.bench_function("bulk_insert_1k_rows", |b: &mut criterion::Bencher| {
        b.iter(|| {
            runtime.block_on(async {
                // Clear orders table
                sqlx::query("TRUNCATE TABLE orders RESTART IDENTITY")
                    .execute(pool)
                    .await
                    .unwrap();

                // Insert 1000 rows
                for i in 1i64..=1000 {
                    let customer_id = (i % 100) + 1;
                    let product_id = (i % 50) + 1;
                    let quantity = ((i % 10) + 1) as i32;
                    let total_price = 100 * quantity;
                    let values = vec![
                        (
                            "customer_id".to_string(),
                            ExtractedValue::BigInt(customer_id),
                        ),
                        ("product_id".to_string(), ExtractedValue::BigInt(product_id)),
                        ("quantity".to_string(), ExtractedValue::Int(quantity)),
                        ("total_price".to_string(), ExtractedValue::Int(total_price)),
                    ];
                    black_box(Row::insert(pool, "orders", &values).await.unwrap());
                }
            })
        });
    });

    runtime.block_on(cleanup_test_tables(&conn));
}

/// Benchmark: Complex Query with JOIN and filters
/// Target: <20ms
fn bench_complex_query(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let conn = runtime.block_on(setup_connection());
    runtime.block_on(setup_test_tables(&conn));

    let pool = conn.pool();

    // Pre-populate orders table with test data
    runtime.block_on(async {
        for i in 1i64..=1000 {
            let customer_id = (i % 100) + 1;
            let product_id = (i % 50) + 1;
            let quantity = ((i % 10) + 1) as i32;
            let total_price = 100 * quantity;
            let values = vec![
                (
                    "customer_id".to_string(),
                    ExtractedValue::BigInt(customer_id),
                ),
                ("product_id".to_string(), ExtractedValue::BigInt(product_id)),
                ("quantity".to_string(), ExtractedValue::Int(quantity)),
                ("total_price".to_string(), ExtractedValue::Int(total_price)),
            ];
            Row::insert(pool, "orders", &values).await.unwrap();
        }
    });

    c.bench_function("complex_query_join_filter", |b: &mut criterion::Bencher| {
        b.iter(|| {
            runtime.block_on(async {
                // Complex query: Join orders with customers and products, filter by price
                let join_cond = JoinCondition::new("id", "orders", "customer_id").unwrap();
                let query = QueryBuilder::new("customers")
                    .unwrap()
                    .select(vec![
                        "customers.name".to_string(),
                        "customers.email".to_string(),
                        "orders.total_price".to_string(),
                    ])
                    .unwrap()
                    .join(JoinType::Inner, "orders", None, join_cond)
                    .unwrap()
                    .where_clause("total_price", Operator::Gte, ExtractedValue::Int(500))
                    .unwrap()
                    .order_by("total_price", OrderDirection::Desc)
                    .unwrap()
                    .limit(100);

                let results = black_box(
                    Row::find_many(pool, "customers", Some(&query))
                        .await
                        .unwrap(),
                );
                assert!(results.len() > 0, "Query should return results");
            })
        });
    });

    runtime.block_on(cleanup_test_tables(&conn));
}

/// Benchmark: Serialization overhead for large result sets
/// Target: <5ms for 10k rows
fn bench_serialization_overhead(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let conn = runtime.block_on(setup_connection());
    runtime.block_on(setup_test_tables(&conn));

    let pool = conn.pool();

    // Pre-populate with 10k orders
    runtime.block_on(async {
        for i in 1i64..=10_000 {
            let customer_id = (i % 100) + 1;
            let product_id = (i % 50) + 1;
            let quantity = ((i % 10) + 1) as i32;
            let total_price = 100 * quantity;
            let values = vec![
                (
                    "customer_id".to_string(),
                    ExtractedValue::BigInt(customer_id),
                ),
                ("product_id".to_string(), ExtractedValue::BigInt(product_id)),
                ("quantity".to_string(), ExtractedValue::Int(quantity)),
                ("total_price".to_string(), ExtractedValue::Int(total_price)),
            ];
            Row::insert(pool, "orders", &values).await.unwrap();
        }
    });

    c.bench_function("serialization_10k_rows", |b: &mut criterion::Bencher| {
        b.iter(|| {
            runtime.block_on(async {
                // Fetch all 10k rows and measure serialization time
                let query = QueryBuilder::new("orders")
                    .unwrap()
                    .select(vec![
                        "id".to_string(),
                        "customer_id".to_string(),
                        "product_id".to_string(),
                        "quantity".to_string(),
                        "total_price".to_string(),
                    ])
                    .unwrap();

                let results =
                    black_box(Row::find_many(pool, "orders", Some(&query)).await.unwrap());
                assert_eq!(results.len(), 10_000, "Should fetch all 10k rows");
            })
        });
    });

    runtime.block_on(cleanup_test_tables(&conn));
}

/// Benchmark: Query Builder construction overhead
fn bench_query_builder_construction(c: &mut Criterion) {
    let mut group = c.benchmark_group("query_builder");

    group.bench_function("simple_query", |b| {
        b.iter(|| {
            black_box(
                QueryBuilder::new("users")
                    .unwrap()
                    .select(vec!["id".to_string(), "name".to_string()])
                    .unwrap()
                    .build(),
            );
        });
    });

    group.bench_function("complex_query", |b| {
        b.iter(|| {
            let join_cond = JoinCondition::new("id", "orders", "user_id").unwrap();
            black_box(
                QueryBuilder::new("users")
                    .unwrap()
                    .select(vec![
                        "users.id".to_string(),
                        "users.name".to_string(),
                        "orders.total".to_string(),
                    ])
                    .unwrap()
                    .join(JoinType::Left, "orders", None, join_cond)
                    .unwrap()
                    .where_clause("age", Operator::Gte, ExtractedValue::Int(18))
                    .unwrap()
                    .order_by("name", OrderDirection::Asc)
                    .unwrap()
                    .limit(10)
                    .build(),
            );
        });
    });

    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .sample_size(10)
        .measurement_time(Duration::from_secs(10))
        .warm_up_time(Duration::from_secs(3));
    targets = bench_bulk_insert, bench_complex_query, bench_serialization_overhead, bench_query_builder_construction
}

criterion_main!(benches);
