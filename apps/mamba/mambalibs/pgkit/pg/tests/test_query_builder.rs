//! Integration tests for QueryBuilder ORM features.
//!
//! Tests all QueryBuilder capabilities: WHERE operators, JOINs, aggregates,
//! window functions, CTEs, and set operations.
//!
//! Run with: cargo test -p cclab-titan --test test_query_builder

use pg::{
    Connection, ExtractedValue, JoinCondition, Operator, OrderDirection, PoolConfig, QueryBuilder,
    WindowSpec,
};

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
// Basic SELECT Tests
// =============================================================================

#[tokio::test]
async fn test_select_all_columns() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_select_all";

    cleanup_table(pool, table).await?;
    sqlx::query(&format!(
        "CREATE TABLE {} (id SERIAL PRIMARY KEY, name TEXT, age INTEGER)",
        table
    ))
    .execute(pool)
    .await?;

    sqlx::query(&format!(
        "INSERT INTO {} (name, age) VALUES ('Alice', 30), ('Bob', 25)",
        table
    ))
    .execute(pool)
    .await?;

    let qb = QueryBuilder::new(table)?;
    let (sql, _params) = qb.build();

    assert!(sql.contains("SELECT *"));
    assert!(sql.contains(&format!("FROM \"{}\"", table)));

    cleanup_table(pool, table).await?;
    Ok(())
}

#[tokio::test]
async fn test_select_specific_columns() -> Result<(), Box<dyn std::error::Error>> {
    let qb = QueryBuilder::new("users")?.select(vec![
        "id".to_string(),
        "name".to_string(),
        "email".to_string(),
    ])?;

    let (sql, _) = qb.build();
    assert!(sql.contains("\"id\""));
    assert!(sql.contains("\"name\""));
    assert!(sql.contains("\"email\""));
    Ok(())
}

#[tokio::test]
async fn test_select_with_only() -> Result<(), Box<dyn std::error::Error>> {
    let qb = QueryBuilder::new("users")?.only(&["id", "name"])?;

    let (sql, _) = qb.build();
    assert!(sql.contains("\"id\""));
    assert!(sql.contains("\"name\""));
    assert!(!sql.contains("SELECT *"));
    Ok(())
}

#[tokio::test]
async fn test_select_with_defer() -> Result<(), Box<dyn std::error::Error>> {
    let qb = QueryBuilder::new("users")?
        .select(vec![
            "id".to_string(),
            "name".to_string(),
            "large_blob".to_string(),
        ])?
        .defer(&["large_blob"])?;

    let (sql, _) = qb.build();
    assert!(sql.contains("\"id\""));
    assert!(sql.contains("\"name\""));
    // large_blob should be deferred (excluded)
    Ok(())
}

// =============================================================================
// WHERE Clause Tests - Basic Operators
// =============================================================================

#[tokio::test]
async fn test_where_eq() -> Result<(), Box<dyn std::error::Error>> {
    let qb =
        QueryBuilder::new("users")?.where_clause("id", Operator::Eq, ExtractedValue::Int(42))?;

    let (sql, params) = qb.build();
    assert!(sql.contains("WHERE"));
    assert!(sql.contains("\"id\" ="));
    assert_eq!(params.len(), 1);
    Ok(())
}

#[tokio::test]
async fn test_where_multiple_conditions() -> Result<(), Box<dyn std::error::Error>> {
    let qb = QueryBuilder::new("users")?
        .where_clause("age", Operator::Gte, ExtractedValue::Int(18))?
        .where_clause("active", Operator::Eq, ExtractedValue::Bool(true))?
        .where_clause(
            "name",
            Operator::Like,
            ExtractedValue::String("%test%".to_string()),
        )?;

    let (sql, params) = qb.build();
    assert!(sql.contains("AND"));
    assert_eq!(params.len(), 3);
    Ok(())
}

#[tokio::test]
async fn test_filter_by_sqlalchemy_style_equality() -> Result<(), Box<dyn std::error::Error>> {
    let qb = QueryBuilder::new("users")?.filter_by(&[
        ("active", ExtractedValue::Bool(true)),
        ("deleted_at", ExtractedValue::Null),
    ])?;

    let (sql, params) = qb.build();
    assert!(sql.contains("\"active\" = $1"));
    assert!(sql.contains("\"deleted_at\" IS NULL"));
    assert_eq!(params, vec![ExtractedValue::Bool(true)]);
    Ok(())
}

#[tokio::test]
async fn test_where_between_sqlalchemy_style_range() -> Result<(), Box<dyn std::error::Error>> {
    let qb = QueryBuilder::new("orders")?
        .where_between("amount", ExtractedValue::Int(10), ExtractedValue::Int(20))?
        .where_not_between(
            "created_day",
            ExtractedValue::Int(1),
            ExtractedValue::Int(7),
        )?;

    let (sql, params) = qb.build();
    assert!(sql.contains("\"amount\" BETWEEN $1 AND $2"));
    assert!(sql.contains("\"created_day\" NOT BETWEEN $3 AND $4"));
    assert_eq!(
        params,
        vec![
            ExtractedValue::Int(10),
            ExtractedValue::Int(20),
            ExtractedValue::Int(1),
            ExtractedValue::Int(7),
        ]
    );
    Ok(())
}

#[tokio::test]
async fn test_where_null_and_not_null() -> Result<(), Box<dyn std::error::Error>> {
    let qb = QueryBuilder::new("users")?
        .where_null("deleted_at")?
        .where_not_null("email")?;

    let (sql, _) = qb.build();
    assert!(sql.contains("IS NULL"));
    assert!(sql.contains("IS NOT NULL"));
    Ok(())
}

#[tokio::test]
async fn test_where_like_ilike() -> Result<(), Box<dyn std::error::Error>> {
    let qb = QueryBuilder::new("users")?
        .where_clause(
            "name",
            Operator::Like,
            ExtractedValue::String("%alice%".to_string()),
        )?
        .where_clause(
            "email",
            Operator::ILike,
            ExtractedValue::String("%@EXAMPLE.COM".to_string()),
        )?;

    let (sql, _) = qb.build();
    assert!(sql.contains("LIKE"));
    assert!(sql.contains("ILIKE"));
    Ok(())
}

#[tokio::test]
async fn test_where_comparison_operators() -> Result<(), Box<dyn std::error::Error>> {
    let qb = QueryBuilder::new("products")?
        .where_clause("price", Operator::Gt, ExtractedValue::Double(100.0))?
        .where_clause("stock", Operator::Lt, ExtractedValue::Int(50))?
        .where_clause("rating", Operator::Gte, ExtractedValue::Double(4.0))?
        .where_clause("discount", Operator::Lte, ExtractedValue::Double(0.5))?
        .where_clause(
            "category",
            Operator::Ne,
            ExtractedValue::String("deprecated".to_string()),
        )?;

    let (sql, params) = qb.build();
    assert!(sql.contains(">") && !sql.contains(">=") || sql.contains("> $"));
    assert!(sql.contains("<"));
    assert!(sql.contains(">="));
    assert!(sql.contains("<="));
    assert!(sql.contains("<>") || sql.contains("!="));
    assert_eq!(params.len(), 5);
    Ok(())
}

// =============================================================================
// WHERE Clause Tests - Subquery Operators
// =============================================================================

#[tokio::test]
async fn test_where_in_subquery() -> Result<(), Box<dyn std::error::Error>> {
    let subquery = QueryBuilder::new("orders")?
        .select(vec!["user_id".to_string()])?
        .where_clause("total", Operator::Gt, ExtractedValue::Double(1000.0))?;

    let qb = QueryBuilder::new("users")?.where_in_subquery("id", subquery)?;

    let (sql, _) = qb.build();
    assert!(sql.contains("IN (SELECT"));
    Ok(())
}

#[tokio::test]
async fn test_where_not_in_subquery() -> Result<(), Box<dyn std::error::Error>> {
    let subquery = QueryBuilder::new("banned_users")?.select(vec!["user_id".to_string()])?;

    let qb = QueryBuilder::new("users")?.where_not_in_subquery("id", subquery)?;

    let (sql, _) = qb.build();
    assert!(sql.contains("NOT IN (SELECT"));
    Ok(())
}

#[tokio::test]
async fn test_where_exists() -> Result<(), Box<dyn std::error::Error>> {
    let subquery = QueryBuilder::new("orders")?.select(vec!["1".to_string()])?;

    let qb = QueryBuilder::new("users")?.where_exists(subquery)?;

    let (sql, _) = qb.build();
    assert!(sql.contains("EXISTS (SELECT"));
    Ok(())
}

#[tokio::test]
async fn test_where_not_exists() -> Result<(), Box<dyn std::error::Error>> {
    let subquery = QueryBuilder::new("spam_reports")?.select(vec!["1".to_string()])?;

    let qb = QueryBuilder::new("messages")?.where_not_exists(subquery)?;

    let (sql, _) = qb.build();
    assert!(sql.contains("NOT EXISTS (SELECT"));
    Ok(())
}

// =============================================================================
// WHERE Clause Tests - JSON Operators
// =============================================================================

#[tokio::test]
async fn test_where_json_contains() -> Result<(), Box<dyn std::error::Error>> {
    let qb =
        QueryBuilder::new("documents")?.where_json_contains("metadata", r#"{"type": "report"}"#)?;

    let (sql, _) = qb.build();
    assert!(sql.contains("@>"));
    Ok(())
}

#[tokio::test]
async fn test_where_json_contained_by() -> Result<(), Box<dyn std::error::Error>> {
    let qb = QueryBuilder::new("documents")?
        .where_json_contained_by("tags", r#"["public", "featured", "new"]"#)?;

    let (sql, _) = qb.build();
    assert!(sql.contains("<@"));
    Ok(())
}

#[tokio::test]
async fn test_where_json_key_exists() -> Result<(), Box<dyn std::error::Error>> {
    let qb = QueryBuilder::new("documents")?.where_json_key_exists("metadata", "author")?;

    let (sql, _) = qb.build();
    assert!(sql.contains("?"));
    Ok(())
}

#[tokio::test]
async fn test_where_json_any_key_exists() -> Result<(), Box<dyn std::error::Error>> {
    let qb = QueryBuilder::new("documents")?
        .where_json_any_key_exists("metadata", &["title", "name", "label"])?;

    let (sql, _) = qb.build();
    assert!(sql.contains("?|"));
    Ok(())
}

#[tokio::test]
async fn test_where_json_all_keys_exist() -> Result<(), Box<dyn std::error::Error>> {
    let qb = QueryBuilder::new("documents")?
        .where_json_all_keys_exist("metadata", &["id", "created_at", "updated_at"])?;

    let (sql, _) = qb.build();
    assert!(sql.contains("?&"));
    Ok(())
}

// =============================================================================
// WHERE Clause Tests - Array Operators
// =============================================================================

#[tokio::test]
async fn test_where_any() -> Result<(), Box<dyn std::error::Error>> {
    let qb = QueryBuilder::new("users")?
        .where_any("roles", ExtractedValue::String("admin".to_string()))?;

    let (sql, _) = qb.build();
    assert!(sql.contains("= ANY"));
    Ok(())
}

#[tokio::test]
async fn test_where_array_contains() -> Result<(), Box<dyn std::error::Error>> {
    let qb = QueryBuilder::new("posts")?.where_array_contains(
        "tags",
        vec![
            ExtractedValue::String("rust".to_string()),
            ExtractedValue::String("postgres".to_string()),
        ],
    )?;

    let (sql, _) = qb.build();
    assert!(sql.contains("@>"));
    Ok(())
}

#[tokio::test]
async fn test_where_array_overlaps() -> Result<(), Box<dyn std::error::Error>> {
    let qb = QueryBuilder::new("posts")?.where_array_overlaps(
        "tags",
        vec![
            ExtractedValue::String("featured".to_string()),
            ExtractedValue::String("popular".to_string()),
        ],
    )?;

    let (sql, _) = qb.build();
    assert!(sql.contains("&&"));
    Ok(())
}

// =============================================================================
// ORDER BY, LIMIT, OFFSET Tests
// =============================================================================

#[tokio::test]
async fn test_order_by_single() -> Result<(), Box<dyn std::error::Error>> {
    let qb = QueryBuilder::new("users")?.order_by("created_at", OrderDirection::Desc)?;

    let (sql, _) = qb.build();
    assert!(sql.contains("ORDER BY"));
    assert!(sql.contains("DESC"));
    Ok(())
}

#[tokio::test]
async fn test_order_by_multiple() -> Result<(), Box<dyn std::error::Error>> {
    let qb = QueryBuilder::new("users")?
        .order_by("last_name", OrderDirection::Asc)?
        .order_by("first_name", OrderDirection::Asc)?
        .order_by("created_at", OrderDirection::Desc)?;

    let (sql, _) = qb.build();
    assert!(sql.contains("ORDER BY"));
    assert!(sql.contains("ASC"));
    assert!(sql.contains("DESC"));
    Ok(())
}

#[tokio::test]
async fn test_limit_offset() -> Result<(), Box<dyn std::error::Error>> {
    let qb = QueryBuilder::new("users")?.limit(10).offset(20);

    let (sql, _) = qb.build();
    assert!(sql.contains("LIMIT"));
    assert!(sql.contains("OFFSET"));
    Ok(())
}

#[tokio::test]
async fn test_distinct() -> Result<(), Box<dyn std::error::Error>> {
    let qb = QueryBuilder::new("users")?
        .select(vec!["country".to_string()])?
        .distinct();

    let (sql, _) = qb.build();
    assert!(sql.contains("DISTINCT"));
    Ok(())
}

#[tokio::test]
async fn test_distinct_on() -> Result<(), Box<dyn std::error::Error>> {
    let qb = QueryBuilder::new("events")?
        .distinct_on(&["user_id"])?
        .order_by("user_id", OrderDirection::Asc)?
        .order_by("created_at", OrderDirection::Desc)?;

    let (sql, _) = qb.build();
    assert!(sql.contains("DISTINCT ON"));
    Ok(())
}

// =============================================================================
// JOIN Tests
// =============================================================================

#[tokio::test]
async fn test_inner_join() -> Result<(), Box<dyn std::error::Error>> {
    let condition = JoinCondition::new("user_id", "u", "id")?;
    let qb = QueryBuilder::new("orders")?.inner_join("users", Some("u"), condition)?;

    let (sql, _) = qb.build();
    assert!(sql.contains("INNER JOIN"));
    assert!(sql.contains("ON"));
    Ok(())
}

#[tokio::test]
async fn test_left_join() -> Result<(), Box<dyn std::error::Error>> {
    let condition = JoinCondition::new("id", "profiles", "user_id")?;
    let qb = QueryBuilder::new("users")?.left_join("profiles", None, condition)?;

    let (sql, _) = qb.build();
    assert!(sql.contains("LEFT JOIN"));
    Ok(())
}

#[tokio::test]
async fn test_right_join() -> Result<(), Box<dyn std::error::Error>> {
    let condition = JoinCondition::new("product_id", "products", "id")?;
    let qb = QueryBuilder::new("orders")?.right_join("products", None, condition)?;

    let (sql, _) = qb.build();
    assert!(sql.contains("RIGHT JOIN"));
    Ok(())
}

#[tokio::test]
async fn test_full_outer_join() -> Result<(), Box<dyn std::error::Error>> {
    let condition = JoinCondition::new("dept_id", "departments", "id")?;
    let qb = QueryBuilder::new("employees")?.full_join("departments", None, condition)?;

    let (sql, _) = qb.build();
    assert!(sql.contains("FULL") || sql.contains("FULL OUTER"));
    Ok(())
}

#[tokio::test]
async fn test_multiple_joins() -> Result<(), Box<dyn std::error::Error>> {
    let user_cond = JoinCondition::new("user_id", "u", "id")?;
    let product_cond = JoinCondition::new("product_id", "p", "id")?;
    let shipping_cond = JoinCondition::new("id", "s", "order_id")?;

    let qb = QueryBuilder::new("orders")?
        .inner_join("users", Some("u"), user_cond)?
        .inner_join("products", Some("p"), product_cond)?
        .left_join("shipping", Some("s"), shipping_cond)?;

    let (sql, _) = qb.build();
    assert!(sql.contains("INNER JOIN"));
    assert!(sql.contains("LEFT JOIN"));
    Ok(())
}

// =============================================================================
// Aggregate Tests
// =============================================================================

#[tokio::test]
async fn test_count_all() -> Result<(), Box<dyn std::error::Error>> {
    let qb = QueryBuilder::new("users")?.count_agg(Some("total"))?;

    let (sql, _) = qb.build();
    assert!(sql.contains("COUNT(*)"));
    assert!(sql.contains("AS"));
    Ok(())
}

#[tokio::test]
async fn test_count_distinct() -> Result<(), Box<dyn std::error::Error>> {
    let qb = QueryBuilder::new("orders")?.count_distinct("user_id", Some("unique_customers"))?;

    let (sql, _) = qb.build();
    assert!(sql.contains("COUNT(DISTINCT"));
    Ok(())
}

#[tokio::test]
async fn test_sum_avg_min_max() -> Result<(), Box<dyn std::error::Error>> {
    let qb = QueryBuilder::new("orders")?
        .sum("amount", Some("total_amount"))?
        .avg("amount", Some("avg_amount"))?
        .min("amount", Some("min_amount"))?
        .max("amount", Some("max_amount"))?;

    let (sql, _) = qb.build();
    assert!(sql.contains("SUM("));
    assert!(sql.contains("AVG("));
    assert!(sql.contains("MIN("));
    assert!(sql.contains("MAX("));
    Ok(())
}

#[tokio::test]
async fn test_group_by() -> Result<(), Box<dyn std::error::Error>> {
    let qb = QueryBuilder::new("orders")?
        .select(vec!["status".to_string()])?
        .count_agg(Some("count"))?
        .sum("amount", Some("total"))?
        .group_by(&["status"])?;

    let (sql, _) = qb.build();
    assert!(sql.contains("GROUP BY"));
    Ok(())
}

#[tokio::test]
async fn test_having() -> Result<(), Box<dyn std::error::Error>> {
    let qb = QueryBuilder::new("orders")?
        .select(vec!["user_id".to_string()])?
        .count_agg(Some("order_count"))?
        .group_by(&["user_id"])?
        .having_count(Operator::Gt, ExtractedValue::Int(5))?;

    let (sql, _) = qb.build();
    assert!(sql.contains("HAVING"));
    assert!(sql.contains("COUNT(*)"));
    Ok(())
}

#[tokio::test]
async fn test_having_sum() -> Result<(), Box<dyn std::error::Error>> {
    let qb = QueryBuilder::new("orders")?
        .select(vec!["user_id".to_string()])?
        .sum("amount", Some("total"))?
        .group_by(&["user_id"])?
        .having_sum("amount", Operator::Gt, ExtractedValue::Double(1000.0))?;

    let (sql, _) = qb.build();
    assert!(sql.contains("HAVING"));
    assert!(sql.contains("SUM("));
    Ok(())
}

// =============================================================================
// Window Function Tests
// =============================================================================

#[tokio::test]
async fn test_row_number() -> Result<(), Box<dyn std::error::Error>> {
    let spec = WindowSpec::new()
        .partition_by(&["department"])
        .order_by("salary", OrderDirection::Desc);

    let qb = QueryBuilder::new("employees")?.row_number(spec, "rank")?;

    let (sql, _) = qb.build();
    assert!(sql.contains("ROW_NUMBER()"));
    assert!(sql.contains("OVER"));
    assert!(sql.contains("PARTITION BY"));
    assert!(sql.contains("ORDER BY"));
    Ok(())
}

#[tokio::test]
async fn test_rank_dense_rank() -> Result<(), Box<dyn std::error::Error>> {
    let spec = WindowSpec::new().order_by("score", OrderDirection::Desc);

    let qb = QueryBuilder::new("players")?
        .rank(spec.clone(), "rank")?
        .dense_rank(spec, "dense_rank")?;

    let (sql, _) = qb.build();
    assert!(sql.contains("RANK()"));
    assert!(sql.contains("DENSE_RANK()"));
    Ok(())
}

#[tokio::test]
async fn test_lag_lead() -> Result<(), Box<dyn std::error::Error>> {
    let spec = WindowSpec::new()
        .partition_by(&["user_id"])
        .order_by("created_at", OrderDirection::Asc);

    let qb = QueryBuilder::new("events")?
        .lag("event_type", Some(1), None, spec.clone(), "prev_event")?
        .lead("event_type", Some(1), None, spec, "next_event")?;

    let (sql, _) = qb.build();
    assert!(sql.contains("LAG("));
    assert!(sql.contains("LEAD("));
    Ok(())
}

// =============================================================================
// CTE (WITH clause) Tests
// =============================================================================

#[tokio::test]
async fn test_simple_cte() -> Result<(), Box<dyn std::error::Error>> {
    let cte_query = QueryBuilder::new("orders")?
        .select(vec!["user_id".to_string()])?
        .sum("amount", Some("total"))?
        .group_by(&["user_id"])?;

    let qb = QueryBuilder::new("user_totals")?
        .with_cte("user_totals", cte_query)?
        .where_clause("total", Operator::Gt, ExtractedValue::Double(1000.0))?;

    let (sql, _) = qb.build();
    assert!(sql.contains("WITH"));
    assert!(sql.contains("AS ("));
    Ok(())
}

#[tokio::test]
async fn test_raw_cte() -> Result<(), Box<dyn std::error::Error>> {
    let qb = QueryBuilder::new("recent_orders")?.with_cte_raw(
        "recent_orders",
        "SELECT * FROM orders WHERE created_at > NOW() - INTERVAL '7 days'",
        vec![],
    )?;

    let (sql, _) = qb.build();
    assert!(sql.contains("WITH"));
    assert!(sql.contains("recent_orders"));
    Ok(())
}

// =============================================================================
// INSERT Tests
// =============================================================================

#[tokio::test]
async fn test_build_insert() -> Result<(), Box<dyn std::error::Error>> {
    let qb = QueryBuilder::new("users")?;
    let values = vec![
        (
            "name".to_string(),
            ExtractedValue::String("Alice".to_string()),
        ),
        ("age".to_string(), ExtractedValue::Int(30)),
        ("active".to_string(), ExtractedValue::Bool(true)),
    ];

    let (sql, params) = qb.build_insert(&values)?;
    assert!(sql.contains("INSERT INTO"));
    assert!(sql.contains("VALUES"));
    assert!(sql.contains("RETURNING"));
    assert_eq!(params.len(), 3);
    Ok(())
}

// =============================================================================
// UPDATE Tests
// =============================================================================

#[tokio::test]
async fn test_build_update() -> Result<(), Box<dyn std::error::Error>> {
    let qb =
        QueryBuilder::new("users")?.where_clause("id", Operator::Eq, ExtractedValue::Int(42))?;

    let values = vec![
        (
            "name".to_string(),
            ExtractedValue::String("Bob".to_string()),
        ),
        (
            "updated_at".to_string(),
            ExtractedValue::String("NOW()".to_string()),
        ),
    ];

    let (sql, params) = qb.build_update(&values)?;
    assert!(sql.contains("UPDATE"));
    assert!(sql.contains("SET"));
    assert!(sql.contains("WHERE"));
    assert!(params.len() >= 2);
    Ok(())
}

// =============================================================================
// DELETE Tests
// =============================================================================

#[tokio::test]
async fn test_build_delete() -> Result<(), Box<dyn std::error::Error>> {
    let qb = QueryBuilder::new("users")?
        .where_clause("deleted_at", Operator::IsNotNull, ExtractedValue::Null)?
        .where_clause("id", Operator::Lt, ExtractedValue::Int(100))?;

    let (sql, params) = qb.build_delete();
    assert!(sql.contains("DELETE FROM"));
    assert!(sql.contains("WHERE"));
    assert!(params.len() >= 1);
    Ok(())
}

// =============================================================================
// RETURNING Tests
// =============================================================================

#[tokio::test]
async fn test_returning_clause() -> Result<(), Box<dyn std::error::Error>> {
    let qb = QueryBuilder::new("users")?.returning(&["id", "created_at"])?;

    let values = vec![(
        "name".to_string(),
        ExtractedValue::String("Test".to_string()),
    )];

    let (sql, _) = qb.build_insert(&values)?;
    assert!(sql.contains("RETURNING"));
    assert!(sql.contains("\"id\""));
    assert!(sql.contains("\"created_at\""));
    Ok(())
}

// =============================================================================
// Integration Tests with Real Database
// =============================================================================

#[tokio::test]
async fn test_integration_select_with_join() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();

    // Cleanup
    cleanup_table(pool, "test_orders_qb").await?;
    cleanup_table(pool, "test_users_qb").await?;

    // Create tables
    sqlx::query("CREATE TABLE test_users_qb (id SERIAL PRIMARY KEY, name TEXT NOT NULL)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE TABLE test_orders_qb (id SERIAL PRIMARY KEY, user_id INTEGER REFERENCES test_users_qb(id), amount NUMERIC(10,2))")
        .execute(pool).await?;

    // Insert data
    sqlx::query("INSERT INTO test_users_qb (name) VALUES ('Alice'), ('Bob')")
        .execute(pool)
        .await?;
    sqlx::query(
        "INSERT INTO test_orders_qb (user_id, amount) VALUES (1, 100.00), (1, 200.00), (2, 150.00)",
    )
    .execute(pool)
    .await?;

    // Build and execute query with JOIN
    let condition = JoinCondition::new("user_id", "test_users_qb", "id")?;
    let qb = QueryBuilder::new("test_orders_qb")?
        .select_raw(vec![
            "test_orders_qb.id".to_string(),
            "test_users_qb.name".to_string(),
            "test_orders_qb.amount".to_string(),
        ])
        .inner_join("test_users_qb", None, condition)?
        .order_by("test_orders_qb.id", OrderDirection::Asc)?;

    let (sql, _params) = qb.build();

    let rows = sqlx::query(&sql).fetch_all(pool).await?;

    assert_eq!(rows.len(), 3);

    // Cleanup
    cleanup_table(pool, "test_orders_qb").await?;
    cleanup_table(pool, "test_users_qb").await?;
    Ok(())
}

#[tokio::test]
async fn test_integration_aggregates() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_agg_qb";

    cleanup_table(pool, table).await?;

    sqlx::query(&format!(
        "CREATE TABLE {} (id SERIAL PRIMARY KEY, category TEXT, amount NUMERIC(10,2))",
        table
    ))
    .execute(pool)
    .await?;

    sqlx::query(&format!(
        "INSERT INTO {} (category, amount) VALUES ('A', 100), ('A', 200), ('B', 150), ('B', 50)",
        table
    ))
    .execute(pool)
    .await?;

    // Test aggregates
    let qb = QueryBuilder::new(table)?
        .select(vec!["category".to_string()])?
        .count_agg(Some("cnt"))?
        .sum("amount", Some("total"))?
        .group_by(&["category"])?
        .order_by("category", OrderDirection::Asc)?;

    let (sql, _) = qb.build();
    let rows = sqlx::query(&sql).fetch_all(pool).await?;

    assert_eq!(rows.len(), 2);

    cleanup_table(pool, table).await?;
    Ok(())
}

#[tokio::test]
async fn test_integration_window_function() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();
    let table = "test_window_qb";

    cleanup_table(pool, table).await?;

    sqlx::query(&format!(
        "CREATE TABLE {} (id SERIAL PRIMARY KEY, dept TEXT, salary INTEGER)",
        table
    ))
    .execute(pool)
    .await?;

    sqlx::query(&format!(
        "INSERT INTO {} (dept, salary) VALUES ('IT', 5000), ('IT', 6000), ('HR', 4000), ('HR', 4500)",
        table
    )).execute(pool).await?;

    // Test window function
    let spec = WindowSpec::new()
        .partition_by(&["dept"])
        .order_by("salary", OrderDirection::Desc);

    let qb = QueryBuilder::new(table)?.row_number(spec, "rank")?;

    let (sql, _) = qb.build();
    let rows = sqlx::query(&sql).fetch_all(pool).await?;

    assert_eq!(rows.len(), 4);

    cleanup_table(pool, table).await?;
    Ok(())
}
