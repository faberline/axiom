//! Integration tests for the blocking façade. Mirror the async
//! `test_migration.rs` shape: each `#[test]` is `#[ignore]` and runs
//! against a real PostgreSQL when `--ignored` is passed.

use std::sync::Arc;

use pg::blocking::{Connection, MigrationRunner, Transaction};
use pg::{IsolationLevel, Migration, PoolConfig};

fn pg_url() -> String {
    std::env::var("POSTGRES_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .unwrap_or_else(|_| "postgresql://localhost/test_db".to_string())
}

#[test]
#[ignore]
fn blocking_connection_ping() {
    let conn = Connection::new(&pg_url(), PoolConfig::default()).expect("connect");
    conn.ping().expect("ping");
}

#[test]
#[ignore]
fn blocking_runtime_is_shared_with_derived_handles() {
    let conn = Connection::new(&pg_url(), PoolConfig::default()).expect("connect");

    // Derived handles must reuse the parent's Arc<Runtime>, not allocate fresh.
    let rt_a = conn.runtime();
    let baseline = Arc::strong_count(&rt_a);

    let _runner = MigrationRunner::new(conn.clone(), Some("_blocking_share_rt".into()));
    assert!(
        Arc::strong_count(&rt_a) > baseline,
        "MigrationRunner did not clone parent runtime Arc"
    );

    let after_runner = Arc::strong_count(&rt_a);
    let tx = Transaction::begin(&conn, IsolationLevel::ReadCommitted).expect("begin");
    assert!(
        Arc::strong_count(&rt_a) > after_runner,
        "Transaction did not clone parent runtime Arc"
    );
    tx.rollback().expect("rollback");
}

#[test]
#[ignore]
fn blocking_migration_apply_then_revert() {
    let conn = Connection::new(&pg_url(), PoolConfig::default()).expect("connect");
    let runner = MigrationRunner::new(conn.clone(), Some("_blocking_test_migrations".into()));
    runner.init().expect("init");

    let migration = Migration::new(
        "20260512_000001".to_string(),
        "blocking test create table".to_string(),
        "CREATE TABLE _blocking_test_table (id SERIAL PRIMARY KEY, name TEXT)".to_string(),
        "DROP TABLE _blocking_test_table".to_string(),
    );

    runner.apply(&migration).expect("apply");
    let applied = runner.applied_migrations().expect("applied list");
    assert!(applied.contains(&"20260512_000001".to_string()));

    runner.revert(&migration).expect("revert");
    let after = runner.applied_migrations().expect("after revert");
    assert!(!after.contains(&"20260512_000001".to_string()));

    // Cleanup tracking table
    let pool = conn.pool();
    let rt = conn.runtime();
    rt.block_on(async {
        sqlx::query("DROP TABLE IF EXISTS _blocking_test_migrations CASCADE")
            .execute(pool)
            .await
            .ok();
    });
}

#[test]
#[ignore]
fn blocking_transaction_commit() {
    let conn = Connection::new(&pg_url(), PoolConfig::default()).expect("connect");
    let rt = conn.runtime();

    // Setup target table via raw sqlx through the shared runtime.
    rt.block_on(async {
        sqlx::query("DROP TABLE IF EXISTS _blocking_txn_test CASCADE")
            .execute(conn.pool())
            .await
            .unwrap();
        sqlx::query("CREATE TABLE _blocking_txn_test (id INT PRIMARY KEY)")
            .execute(conn.pool())
            .await
            .unwrap();
    });

    let mut tx = Transaction::begin(&conn, IsolationLevel::ReadCommitted).expect("begin");

    // Insert + savepoint + rollback to savepoint
    rt.block_on(async {
        sqlx::query("INSERT INTO _blocking_txn_test (id) VALUES (1)")
            .execute(&mut **tx.as_mut_transaction())
            .await
            .unwrap();
    });
    tx.savepoint("sp1").expect("savepoint");
    rt.block_on(async {
        sqlx::query("INSERT INTO _blocking_txn_test (id) VALUES (2)")
            .execute(&mut **tx.as_mut_transaction())
            .await
            .unwrap();
    });
    tx.rollback_to("sp1").expect("rollback_to");
    tx.commit().expect("commit");

    // Only id=1 should remain.
    let count: i64 = rt.block_on(async {
        sqlx::query_scalar("SELECT COUNT(*) FROM _blocking_txn_test")
            .fetch_one(conn.pool())
            .await
            .unwrap()
    });
    assert_eq!(count, 1);

    rt.block_on(async {
        sqlx::query("DROP TABLE _blocking_txn_test")
            .execute(conn.pool())
            .await
            .ok();
    });
}

#[test]
#[ignore]
fn blocking_transaction_rollback_discards_writes() {
    let conn = Connection::new(&pg_url(), PoolConfig::default()).expect("connect");
    let rt = conn.runtime();

    rt.block_on(async {
        sqlx::query("DROP TABLE IF EXISTS _blocking_rollback_test CASCADE")
            .execute(conn.pool())
            .await
            .unwrap();
        sqlx::query("CREATE TABLE _blocking_rollback_test (id INT PRIMARY KEY)")
            .execute(conn.pool())
            .await
            .unwrap();
    });

    let mut tx = Transaction::begin(&conn, IsolationLevel::ReadCommitted).expect("begin");
    rt.block_on(async {
        sqlx::query("INSERT INTO _blocking_rollback_test (id) VALUES (42)")
            .execute(&mut **tx.as_mut_transaction())
            .await
            .unwrap();
    });
    tx.rollback().expect("rollback");

    let count: i64 = rt.block_on(async {
        sqlx::query_scalar("SELECT COUNT(*) FROM _blocking_rollback_test")
            .fetch_one(conn.pool())
            .await
            .unwrap()
    });
    assert_eq!(count, 0);

    rt.block_on(async {
        sqlx::query("DROP TABLE _blocking_rollback_test")
            .execute(conn.pool())
            .await
            .ok();
    });
}
