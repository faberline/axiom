//! A fixed smoke test for authored Rust oracle fixtures.
//!
//! This binary accepts no source text. Python calls `cargo test --offline
//! --locked` on this crate during data preparation.

fn repair(value: Result<i32, &'static str>) -> Result<i32, &'static str> {
    value
}

fn main() {
    assert_eq!(repair(Ok(2)), Ok(2));
    assert_eq!(repair(Err("db")), Err("db"));
}

#[cfg(test)]
mod tests {
    use super::repair;
    use axum::{routing::get, Router};
    use serde::{Deserialize, Serialize};
    use sqlx::sqlite::SqlitePoolOptions;

    #[test]
    fn preserves_success_and_error() {
        assert_eq!(repair(Ok(7)), Ok(7));
        assert_eq!(repair(Err("timeout")), Err("timeout"));
    }

    #[derive(Debug, Deserialize, Serialize, PartialEq)]
    struct Payload { name: Option<String> }

    #[tokio::test]
    async fn fixed_backend_stack_smoke_uses_local_sqlite() {
        let pool = SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
        sqlx::query("create table items (id integer primary key)").execute(&pool).await.unwrap();
        sqlx::query("insert into items (id) values (7)").execute(&pool).await.unwrap();
        let row: (i64,) = sqlx::query_as("select id from items").fetch_one(&pool).await.unwrap();
        assert_eq!(row.0, 7);
        assert_eq!(Payload { name: Some("ok".into()) }, serde_json::from_str(r#"{"name":"ok"}"#).unwrap());
        let _router: Router = Router::new().route("/health", get(|| async { "ok" }));
    }
}
