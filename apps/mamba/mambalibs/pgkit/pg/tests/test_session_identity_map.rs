//! Integration tests for [`pg::orm::Session`].
//!
//! Each test bootstraps a `test_session_users` table, exercises the
//! Session semantics, and drops the table on teardown. Tests are
//! gated on `DATABASE_URL` — they skip silently when unset so CI
//! environments without Postgres stay green.
//!
//! @spec
//!   .aw/tech-design/projects/pg/specs/pg-orm-session-uow.md#changes

// HANDWRITE-BEGIN reason: integration tests are inherently
//   hand-written until codegen learns a `test-plan-integration`
//   section type.

use std::sync::Arc;

use pg::orm::session::sealed::Sealed;
use pg::{
    Connection, DataBridgeError, ExtractedValue, Operator, PoolConfig, Result, Row, Session,
    SessionModel,
};

#[derive(Debug, Clone)]
struct TestUser {
    id: i64,
    name: String,
    email: String,
}

impl Sealed for TestUser {}

impl SessionModel for TestUser {
    fn table() -> &'static str {
        "test_session_users"
    }
    fn pk(&self) -> i64 {
        self.id
    }
    fn to_values(&self) -> Vec<(String, ExtractedValue)> {
        vec![
            ("name".into(), ExtractedValue::String(self.name.clone())),
            ("email".into(), ExtractedValue::String(self.email.clone())),
        ]
    }
    fn from_row(row: &Row) -> Result<Self> {
        let id = match row.get("id")? {
            ExtractedValue::BigInt(v) => *v,
            other => {
                return Err(DataBridgeError::Query(format!(
                    "test_session_users.id has unexpected type {:?}",
                    other.pg_type_name()
                )))
            }
        };
        let name = match row.get("name")? {
            ExtractedValue::String(s) => s.clone(),
            other => {
                return Err(DataBridgeError::Query(format!(
                    "test_session_users.name has unexpected type {:?}",
                    other.pg_type_name()
                )))
            }
        };
        let email = match row.get("email")? {
            ExtractedValue::String(s) => s.clone(),
            other => {
                return Err(DataBridgeError::Query(format!(
                    "test_session_users.email has unexpected type {:?}",
                    other.pg_type_name()
                )))
            }
        };
        Ok(TestUser { id, name, email })
    }
}

async fn maybe_conn() -> Option<Connection> {
    let uri = std::env::var("DATABASE_URL").ok()?;
    if uri.is_empty() {
        return None;
    }
    Connection::new(&uri, PoolConfig::default()).await.ok()
}

async fn bootstrap(conn: &Connection) {
    let pool = conn.pool();
    sqlx::query("DROP TABLE IF EXISTS test_session_users CASCADE")
        .execute(pool)
        .await
        .expect("drop");
    sqlx::query(
        "CREATE TABLE test_session_users (
            id BIGSERIAL PRIMARY KEY,
            name TEXT NOT NULL,
            email TEXT NOT NULL
        )",
    )
    .execute(pool)
    .await
    .expect("create");
}

async fn teardown(conn: &Connection) {
    let pool = conn.pool();
    let _ = sqlx::query("DROP TABLE IF EXISTS test_session_users CASCADE")
        .execute(pool)
        .await;
}

#[tokio::test]
async fn identity_map_repeat_get_returns_same_arc() {
    let Some(conn) = maybe_conn().await else {
        return;
    };
    bootstrap(&conn).await;
    let row = Row::insert(
        conn.pool(),
        "test_session_users",
        &[
            ("name".into(), ExtractedValue::String("Alice".into())),
            ("email".into(), ExtractedValue::String("a@e".into())),
        ],
    )
    .await
    .expect("insert");
    let pk = match row.get("id").unwrap() {
        ExtractedValue::BigInt(v) => *v,
        _ => panic!("id"),
    };

    let mut s = Session::new(&conn);
    let first = s.get::<TestUser>(pk).await.unwrap().expect("first");
    let second = s.get::<TestUser>(pk).await.unwrap().expect("second");
    assert!(
        Arc::ptr_eq(&first, &second),
        "identity-map must return the same Arc on repeat get",
    );

    teardown(&conn).await;
}

#[tokio::test]
async fn staging_order_preserved_through_flush() {
    let Some(conn) = maybe_conn().await else {
        return;
    };
    bootstrap(&conn).await;

    let mut s = Session::new(&conn);
    s.add(Arc::new(TestUser {
        id: 0,
        name: "u_a".into(),
        email: "u_a@e".into(),
    }));
    s.add(Arc::new(TestUser {
        id: 0,
        name: "u_b".into(),
        email: "u_b@e".into(),
    }));
    s.commit().await.expect("commit");

    let rows = Row::find_many(conn.pool(), "test_session_users", None)
        .await
        .expect("find_many");
    let mut names_in_id_order: Vec<(i64, String)> = rows
        .iter()
        .map(|r| {
            let id = match r.get("id").unwrap() {
                ExtractedValue::BigInt(v) => *v,
                _ => panic!(),
            };
            let name = match r.get("name").unwrap() {
                ExtractedValue::String(s) => s.clone(),
                _ => panic!(),
            };
            (id, name)
        })
        .collect();
    names_in_id_order.sort_by_key(|(id, _)| *id);
    let names: Vec<String> = names_in_id_order.into_iter().map(|(_, n)| n).collect();
    assert_eq!(names, vec!["u_a".to_string(), "u_b".to_string()]);

    teardown(&conn).await;
}

#[tokio::test]
async fn flush_canonical_order_insert_update_delete() {
    let Some(conn) = maybe_conn().await else {
        return;
    };
    bootstrap(&conn).await;

    let pool = conn.pool();
    let seed_update = Row::insert(
        pool,
        "test_session_users",
        &[
            ("name".into(), ExtractedValue::String("to_update".into())),
            ("email".into(), ExtractedValue::String("u@e".into())),
        ],
    )
    .await
    .unwrap();
    let pk_u = match seed_update.get("id").unwrap() {
        ExtractedValue::BigInt(v) => *v,
        _ => panic!(),
    };
    let seed_delete = Row::insert(
        pool,
        "test_session_users",
        &[
            ("name".into(), ExtractedValue::String("to_delete".into())),
            ("email".into(), ExtractedValue::String("d@e".into())),
        ],
    )
    .await
    .unwrap();
    let pk_d = match seed_delete.get("id").unwrap() {
        ExtractedValue::BigInt(v) => *v,
        _ => panic!(),
    };

    let mut s = Session::new(&conn);
    s.add(Arc::new(TestUser {
        id: 0,
        name: "new_row".into(),
        email: "n@e".into(),
    }));
    s.touch(Arc::new(TestUser {
        id: pk_u,
        name: "updated".into(),
        email: "u2@e".into(),
    }));
    s.delete::<TestUser>(pk_d);
    s.commit().await.expect("commit");

    let all = Row::find_many(pool, "test_session_users", None)
        .await
        .unwrap();
    let names: std::collections::HashSet<String> = all
        .iter()
        .map(|r| match r.get("name").unwrap() {
            ExtractedValue::String(s) => s.clone(),
            _ => panic!(),
        })
        .collect();
    assert!(names.contains("new_row"), "INSERT must have landed");
    assert!(names.contains("updated"), "UPDATE must have landed");
    assert!(!names.contains("to_delete"), "DELETE must have landed");
    assert!(
        !names.contains("to_update"),
        "old name must be gone after UPDATE",
    );

    teardown(&conn).await;
}

#[tokio::test]
async fn commit_clears_staging_keeps_identity_map() {
    let Some(conn) = maybe_conn().await else {
        return;
    };
    bootstrap(&conn).await;

    let row = Row::insert(
        conn.pool(),
        "test_session_users",
        &[
            ("name".into(), ExtractedValue::String("Alice".into())),
            ("email".into(), ExtractedValue::String("a@e".into())),
        ],
    )
    .await
    .unwrap();
    let pk = match row.get("id").unwrap() {
        ExtractedValue::BigInt(v) => *v,
        _ => panic!(),
    };

    let mut s = Session::new(&conn);
    let cached = s.get::<TestUser>(pk).await.unwrap().unwrap();
    s.touch(Arc::new(TestUser {
        id: pk,
        name: "Alice2".into(),
        email: cached.email.clone(),
    }));
    assert_eq!(s.staging_len(), 1);
    assert!(s.identity_map_len() >= 1);

    s.commit().await.expect("commit");
    assert_eq!(s.staging_len(), 0, "commit must drain staging");
    assert!(
        s.identity_map_len() >= 1,
        "commit must not clear the identity map",
    );

    teardown(&conn).await;
}

#[tokio::test]
async fn rollback_clears_both_staging_and_identity_map() {
    let Some(conn) = maybe_conn().await else {
        return;
    };
    bootstrap(&conn).await;

    let row = Row::insert(
        conn.pool(),
        "test_session_users",
        &[
            ("name".into(), ExtractedValue::String("Alice".into())),
            ("email".into(), ExtractedValue::String("a@e".into())),
        ],
    )
    .await
    .unwrap();
    let pk = match row.get("id").unwrap() {
        ExtractedValue::BigInt(v) => *v,
        _ => panic!(),
    };

    let mut s = Session::new(&conn);
    s.begin().await.expect("begin");
    let _cached = s.get::<TestUser>(pk).await.unwrap().unwrap();
    s.add(Arc::new(TestUser {
        id: 0,
        name: "tx_only".into(),
        email: "t@e".into(),
    }));
    assert_eq!(s.staging_len(), 1);
    assert!(s.identity_map_len() >= 1);

    s.rollback().await.expect("rollback");
    assert_eq!(s.staging_len(), 0, "rollback must clear staging");
    assert_eq!(s.identity_map_len(), 0, "rollback must clear identity map");

    let rows = Row::find_many(conn.pool(), "test_session_users", None)
        .await
        .unwrap();
    assert_eq!(rows.len(), 1, "INSERT staged before rollback must not land");

    teardown(&conn).await;
}

#[tokio::test]
async fn query_routes_through_identity_map() {
    let Some(conn) = maybe_conn().await else {
        return;
    };
    bootstrap(&conn).await;

    let pool = conn.pool();
    let r1 = Row::insert(
        pool,
        "test_session_users",
        &[
            ("name".into(), ExtractedValue::String("Alice".into())),
            ("email".into(), ExtractedValue::String("a@e".into())),
        ],
    )
    .await
    .unwrap();
    let _r2 = Row::insert(
        pool,
        "test_session_users",
        &[
            ("name".into(), ExtractedValue::String("Bob".into())),
            ("email".into(), ExtractedValue::String("b@e".into())),
        ],
    )
    .await
    .unwrap();
    let pk_alice = match r1.get("id").unwrap() {
        ExtractedValue::BigInt(v) => *v,
        _ => panic!(),
    };

    let mut s = Session::new(&conn);
    let from_query: Vec<Arc<TestUser>> = s
        .query::<TestUser>()
        .unwrap()
        .filter("name", Operator::Eq, ExtractedValue::String("Alice".into()))
        .unwrap()
        .all()
        .await
        .unwrap();
    assert_eq!(from_query.len(), 1);
    let from_query = from_query.into_iter().next().unwrap();

    let from_get = s.get::<TestUser>(pk_alice).await.unwrap().unwrap();
    assert!(
        Arc::ptr_eq(&from_query, &from_get),
        "query() and get() must return the same Arc for the same pk",
    );

    teardown(&conn).await;
}

// HANDWRITE-END
