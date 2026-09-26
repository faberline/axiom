//! Integration tests for Row relation methods (eager loading).
//!
//! These tests verify the find_with_relations, find_many_with_relations,
//! and find_one_eager methods that eliminate N+1 query problems.
//!
//! These tests require a PostgreSQL database to be running.
//! Set DATABASE_URL environment variable or skip with SKIP_INTEGRATION=true

use pg::{Connection, ExtractedValue, JoinType, PoolConfig, RelationConfig, Row};
use qc::{expect, AssertionError};

#[tokio::test]
#[ignore] // Only run with --ignored flag when database is available
async fn test_find_with_relations_basic() -> Result<(), AssertionError> {
    // Setup database connection
    let uri = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://localhost/test_db".to_string());

    let conn = Connection::new(&uri, PoolConfig::default()).await.unwrap();
    let pool = conn.pool();

    // Create test tables
    sqlx::query("DROP TABLE IF EXISTS test_posts CASCADE")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DROP TABLE IF EXISTS test_users CASCADE")
        .execute(pool)
        .await
        .unwrap();

    sqlx::query(
        "CREATE TABLE test_users (
            id BIGSERIAL PRIMARY KEY,
            name TEXT NOT NULL
        )",
    )
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "CREATE TABLE test_posts (
            id BIGSERIAL PRIMARY KEY,
            title TEXT NOT NULL,
            author_id BIGINT REFERENCES test_users(id)
        )",
    )
    .execute(pool)
    .await
    .unwrap();

    // Insert test data
    let user_values = vec![(
        "name".to_string(),
        ExtractedValue::String("Alice".to_string()),
    )];
    let user = Row::insert(pool, "test_users", &user_values).await.unwrap();
    let user_id = match user.get("id").unwrap() {
        ExtractedValue::BigInt(i) => *i,
        _ => panic!("Expected BigInt for id"),
    };

    let post_values = vec![
        (
            "title".to_string(),
            ExtractedValue::String("First Post".to_string()),
        ),
        ("author_id".to_string(), ExtractedValue::BigInt(user_id)),
    ];
    let post = Row::insert(pool, "test_posts", &post_values).await.unwrap();
    let post_id = match post.get("id").unwrap() {
        ExtractedValue::BigInt(i) => *i,
        _ => panic!("Expected BigInt for id"),
    };

    // Test find_with_relations
    let relations = vec![RelationConfig {
        name: "author".to_string(),
        table: "test_users".to_string(),
        foreign_key: "author_id".to_string(),
        reference_column: "id".to_string(),
        join_type: JoinType::Left,
        select_columns: Some(vec!["id".to_string(), "name".to_string()]),
    }];

    let result = Row::find_with_relations(pool, "test_posts", post_id, &relations)
        .await
        .unwrap()
        .expect("Post should exist");

    // Verify main row data
    expect(matches!(
        result.get("title").unwrap(),
        ExtractedValue::String(s) if s == "First Post"
    ))
    .to_be_true()?;

    // Verify relation data
    let author_data = result.get("author").unwrap();
    match author_data {
        ExtractedValue::Json(json_val) => {
            let author_obj = json_val.as_object().expect("Author should be object");
            expect(author_obj.get("name").unwrap().as_str().unwrap()).to_equal(&"Alice")?;
            expect(author_obj.get("id").unwrap().as_i64().unwrap()).to_equal(&user_id)?;
        }
        _ => panic!("Expected JSON for author"),
    }

    // Clean up
    sqlx::query("DROP TABLE test_posts CASCADE")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DROP TABLE test_users CASCADE")
        .execute(pool)
        .await
        .unwrap();

    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_find_one_eager_helper() -> Result<(), AssertionError> {
    // Setup database connection
    let uri = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://localhost/test_db".to_string());

    let conn = Connection::new(&uri, PoolConfig::default()).await.unwrap();
    let pool = conn.pool();

    // Create test tables
    sqlx::query("DROP TABLE IF EXISTS test_comments CASCADE")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DROP TABLE IF EXISTS test_posts CASCADE")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DROP TABLE IF EXISTS test_users CASCADE")
        .execute(pool)
        .await
        .unwrap();

    sqlx::query(
        "CREATE TABLE test_users (
            id BIGSERIAL PRIMARY KEY,
            name TEXT NOT NULL
        )",
    )
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "CREATE TABLE test_posts (
            id BIGSERIAL PRIMARY KEY,
            title TEXT NOT NULL,
            author_id BIGINT REFERENCES test_users(id)
        )",
    )
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "CREATE TABLE test_comments (
            id BIGSERIAL PRIMARY KEY,
            content TEXT NOT NULL,
            post_id BIGINT REFERENCES test_posts(id),
            author_id BIGINT REFERENCES test_users(id)
        )",
    )
    .execute(pool)
    .await
    .unwrap();

    // Insert test data
    let user_values = vec![(
        "name".to_string(),
        ExtractedValue::String("Bob".to_string()),
    )];
    let user = Row::insert(pool, "test_users", &user_values).await.unwrap();
    let user_id = match user.get("id").unwrap() {
        ExtractedValue::BigInt(i) => *i,
        _ => panic!("Expected BigInt for id"),
    };

    let post_values = vec![
        (
            "title".to_string(),
            ExtractedValue::String("Test Post".to_string()),
        ),
        ("author_id".to_string(), ExtractedValue::BigInt(user_id)),
    ];
    let post = Row::insert(pool, "test_posts", &post_values).await.unwrap();
    let post_id = match post.get("id").unwrap() {
        ExtractedValue::BigInt(i) => *i,
        _ => panic!("Expected BigInt for id"),
    };

    let comment_values = vec![
        (
            "content".to_string(),
            ExtractedValue::String("Great post!".to_string()),
        ),
        ("post_id".to_string(), ExtractedValue::BigInt(post_id)),
        ("author_id".to_string(), ExtractedValue::BigInt(user_id)),
    ];
    let comment = Row::insert(pool, "test_comments", &comment_values)
        .await
        .unwrap();
    let comment_id = match comment.get("id").unwrap() {
        ExtractedValue::BigInt(i) => *i,
        _ => panic!("Expected BigInt for id"),
    };

    // Test find_one_eager with multiple relations
    let result = Row::find_one_eager(
        pool,
        "test_comments",
        comment_id,
        &[
            ("post", "post_id", "test_posts"),
            ("author", "author_id", "test_users"),
        ],
    )
    .await
    .unwrap()
    .expect("Comment should exist");

    // Verify main row data
    expect(matches!(
        result.get("content").unwrap(),
        ExtractedValue::String(s) if s == "Great post!"
    ))
    .to_be_true()?;

    // Verify post relation
    let post_data = result.get("post").unwrap();
    match post_data {
        ExtractedValue::Json(json_val) => {
            let post_obj = json_val.as_object().expect("Post should be object");
            expect(post_obj.get("title").unwrap().as_str().unwrap()).to_equal(&"Test Post")?;
        }
        _ => panic!("Expected JSON for post"),
    }

    // Verify author relation
    let author_data = result.get("author").unwrap();
    match author_data {
        ExtractedValue::Json(json_val) => {
            let author_obj = json_val.as_object().expect("Author should be object");
            expect(author_obj.get("name").unwrap().as_str().unwrap()).to_equal(&"Bob")?;
        }
        _ => panic!("Expected JSON for author"),
    }

    // Clean up
    sqlx::query("DROP TABLE test_comments CASCADE")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DROP TABLE test_posts CASCADE")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DROP TABLE test_users CASCADE")
        .execute(pool)
        .await
        .unwrap();

    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_find_with_relations_not_found() -> Result<(), AssertionError> {
    // Setup database connection
    let uri = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://localhost/test_db".to_string());

    let conn = Connection::new(&uri, PoolConfig::default()).await.unwrap();
    let pool = conn.pool();

    // Create test tables
    sqlx::query("DROP TABLE IF EXISTS test_posts CASCADE")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DROP TABLE IF EXISTS test_users CASCADE")
        .execute(pool)
        .await
        .unwrap();

    sqlx::query(
        "CREATE TABLE test_users (
            id BIGSERIAL PRIMARY KEY,
            name TEXT NOT NULL
        )",
    )
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "CREATE TABLE test_posts (
            id BIGSERIAL PRIMARY KEY,
            title TEXT NOT NULL,
            author_id BIGINT REFERENCES test_users(id)
        )",
    )
    .execute(pool)
    .await
    .unwrap();

    // Test finding non-existent post
    let relations = vec![RelationConfig {
        name: "author".to_string(),
        table: "test_users".to_string(),
        foreign_key: "author_id".to_string(),
        reference_column: "id".to_string(),
        join_type: JoinType::Left,
        select_columns: None,
    }];

    let result = Row::find_with_relations(pool, "test_posts", 99999, &relations)
        .await
        .unwrap();

    expect(result.is_none()).to_be_true()?;

    // Clean up
    sqlx::query("DROP TABLE test_posts CASCADE")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DROP TABLE test_users CASCADE")
        .execute(pool)
        .await
        .unwrap();

    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_find_with_relations_null_foreign_key() -> Result<(), AssertionError> {
    // Setup database connection
    let uri = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://localhost/test_db".to_string());

    let conn = Connection::new(&uri, PoolConfig::default()).await.unwrap();
    let pool = conn.pool();

    // Create test tables
    sqlx::query("DROP TABLE IF EXISTS test_posts CASCADE")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DROP TABLE IF EXISTS test_users CASCADE")
        .execute(pool)
        .await
        .unwrap();

    sqlx::query(
        "CREATE TABLE test_users (
            id BIGSERIAL PRIMARY KEY,
            name TEXT NOT NULL
        )",
    )
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "CREATE TABLE test_posts (
            id BIGSERIAL PRIMARY KEY,
            title TEXT NOT NULL,
            author_id BIGINT REFERENCES test_users(id)
        )",
    )
    .execute(pool)
    .await
    .unwrap();

    // Insert post with NULL author_id
    let post_values = vec![(
        "title".to_string(),
        ExtractedValue::String("Anonymous Post".to_string()),
    )];
    let post = Row::insert(pool, "test_posts", &post_values).await.unwrap();
    let post_id = match post.get("id").unwrap() {
        ExtractedValue::BigInt(i) => *i,
        _ => panic!("Expected BigInt for id"),
    };

    // Test find_with_relations with NULL foreign key
    let relations = vec![RelationConfig {
        name: "author".to_string(),
        table: "test_users".to_string(),
        foreign_key: "author_id".to_string(),
        reference_column: "id".to_string(),
        join_type: JoinType::Left,
        select_columns: None,
    }];

    let result = Row::find_with_relations(pool, "test_posts", post_id, &relations)
        .await
        .unwrap()
        .expect("Post should exist");

    // Verify main row data
    expect(matches!(
        result.get("title").unwrap(),
        ExtractedValue::String(s) if s == "Anonymous Post"
    ))
    .to_be_true()?;

    // Verify author is NULL (LEFT JOIN with NULL foreign key)
    expect(matches!(
        result.get("author_id").unwrap(),
        ExtractedValue::Null
    ))
    .to_be_true()?;

    // Clean up
    sqlx::query("DROP TABLE test_posts CASCADE")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DROP TABLE test_users CASCADE")
        .execute(pool)
        .await
        .unwrap();

    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_find_with_relations_column_collision() -> Result<(), AssertionError> {
    // Test that main table columns don't collide with relation columns
    // when both tables have columns with the same name (e.g., "id", "created_at")

    let uri = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://localhost/test_db".to_string());

    let conn = Connection::new(&uri, PoolConfig::default()).await.unwrap();
    let pool = conn.pool();

    // Create test tables with overlapping column names
    sqlx::query("DROP TABLE IF EXISTS test_posts CASCADE")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DROP TABLE IF EXISTS test_authors CASCADE")
        .execute(pool)
        .await
        .unwrap();

    sqlx::query(
        "CREATE TABLE test_authors (
            id BIGSERIAL PRIMARY KEY,
            name TEXT NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )",
    )
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "CREATE TABLE test_posts (
            id BIGSERIAL PRIMARY KEY,
            title TEXT NOT NULL,
            author_id BIGINT REFERENCES test_authors(id),
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )",
    )
    .execute(pool)
    .await
    .unwrap();

    // Insert test data
    let author_values = vec![(
        "name".to_string(),
        ExtractedValue::String("Jane Doe".to_string()),
    )];
    let author = Row::insert(pool, "test_authors", &author_values)
        .await
        .unwrap();
    let author_id = match author.get("id").unwrap() {
        ExtractedValue::BigInt(i) => *i,
        _ => panic!("Expected BigInt for id"),
    };

    let post_values = vec![
        (
            "title".to_string(),
            ExtractedValue::String("Collision Test".to_string()),
        ),
        ("author_id".to_string(), ExtractedValue::BigInt(author_id)),
    ];
    let post = Row::insert(pool, "test_posts", &post_values).await.unwrap();
    let post_id = match post.get("id").unwrap() {
        ExtractedValue::BigInt(i) => *i,
        _ => panic!("Expected BigInt for id"),
    };

    // Test find_with_relations with select_columns that include "id" and "created_at"
    let relations = vec![RelationConfig {
        name: "author".to_string(),
        table: "test_authors".to_string(),
        foreign_key: "author_id".to_string(),
        reference_column: "id".to_string(),
        join_type: JoinType::Left,
        select_columns: Some(vec![
            "id".to_string(),
            "name".to_string(),
            "created_at".to_string(),
        ]),
    }];

    let result = Row::find_with_relations(pool, "test_posts", post_id, &relations)
        .await
        .unwrap()
        .expect("Post should exist");

    // Verify main row data - should have the post's id and created_at
    let main_id = match result.get("id").unwrap() {
        ExtractedValue::BigInt(i) => *i,
        _ => panic!("Expected BigInt for main id"),
    };
    expect(main_id).to_equal(&post_id)?;

    expect(matches!(
        result.get("title").unwrap(),
        ExtractedValue::String(s) if s == "Collision Test"
    ))
    .to_be_true()?;

    // Main row should have created_at
    expect(matches!(
        result.get("created_at").unwrap(),
        ExtractedValue::TimestampTz(_)
    ))
    .to_be_true()?;

    // Verify relation data - should have the author's id, name, and created_at
    let author_data = result.get("author").unwrap();
    match author_data {
        ExtractedValue::Json(json_val) => {
            let author_obj = json_val.as_object().expect("Author should be object");

            // Author should have its own id
            let author_rel_id = author_obj.get("id").unwrap().as_i64().unwrap();
            expect(author_rel_id).to_equal(&author_id)?;

            // Author should have name
            expect(author_obj.get("name").unwrap().as_str().unwrap()).to_equal(&"Jane Doe")?;

            // Author should have its own created_at (different from post's created_at)
            expect(author_obj.contains_key("created_at")).to_be_true()?;
        }
        _ => panic!("Expected JSON for author"),
    }

    // Clean up
    sqlx::query("DROP TABLE test_posts CASCADE")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DROP TABLE test_authors CASCADE")
        .execute(pool)
        .await
        .unwrap();

    Ok(())
}
