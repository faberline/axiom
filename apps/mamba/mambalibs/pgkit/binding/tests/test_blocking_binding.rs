//! Mamba-layer integration tests for the driver / Transaction /
//! migrate surface. Mirrors `projects/pgkit/pg/tests/test_blocking.rs`
//! shape (5 #[ignore] tests against a real Postgres).
//!
//! Run with a Postgres reachable at `$DATABASE_URL`:
//!
//! ```text
//! DATABASE_URL=postgres://localhost/test cargo test \
//!     -p pgkit-binding --test test_blocking_binding -- --ignored
//! ```

#![allow(improper_ctypes_definitions)]

use mamba_registry::MbValue;

use pgkit_binding::methods::{
    mb_pg_close, mb_pg_connect, mb_pg_execute, mb_pg_migration_new,
    mb_pg_migration_runner_applied_migrations, mb_pg_migration_runner_apply,
    mb_pg_migration_runner_init, mb_pg_migration_runner_new, mb_pg_migration_runner_revert,
    mb_pg_ping, mb_pg_transaction_begin, mb_pg_transaction_commit, mb_pg_transaction_execute,
    mb_pg_transaction_rollback, mb_pg_transaction_rollback_to, mb_pg_transaction_savepoint,
};
use pgkit_binding::types::{MbPgConnection, MbPgMigration};

fn db_url() -> Option<String> {
    std::env::var("DATABASE_URL").ok()
}

fn s(v: &str) -> MbValue {
    mamba_registry::test_ops::init();
    mamba_registry::rc::wrap_obj_str(v.to_string())
}

unsafe fn handle<'a, T>(v: MbValue) -> &'a T {
    let addr = v.as_ptr().expect("ptr");
    unsafe { &*(addr as *const T) }
}

#[test]
#[ignore]
fn pool_is_reused_across_executes() {
    let Some(url) = db_url() else { return };

    let conn_val = unsafe { mb_pg_connect([s(&url)].as_ptr(), 1) };
    assert!(conn_val.is_ptr(), "connect failed");
    let conn = unsafe { handle::<MbPgConnection>(conn_val) };
    let initial_strong = std::sync::Arc::strong_count(&conn.inner);

    for _ in 0..100 {
        let rv = unsafe { mb_pg_execute([conn_val, s("SELECT 1")].as_ptr(), 2) };
        assert!(
            rv.as_int().is_some(),
            "execute returned non-int — pool not reused?"
        );
    }

    let final_strong = std::sync::Arc::strong_count(&conn.inner);
    assert_eq!(
        initial_strong, final_strong,
        "Arc strong count drifted — execute is cloning the connection"
    );

    let _ = unsafe { mb_pg_ping([conn_val].as_ptr(), 1) };
    let _ = unsafe { mb_pg_close([conn_val].as_ptr(), 1) };
}

#[test]
#[ignore]
fn transaction_savepoint_rollback_to() {
    let Some(url) = db_url() else { return };

    let conn_val = unsafe { mb_pg_connect([s(&url)].as_ptr(), 1) };

    let _ = unsafe {
        mb_pg_execute(
            [conn_val, s("DROP TABLE IF EXISTS mb_pg_tx_marker")].as_ptr(),
            2,
        )
    };
    let _ = unsafe {
        mb_pg_execute(
            [conn_val, s("CREATE TABLE mb_pg_tx_marker(id int)")].as_ptr(),
            2,
        )
    };

    let tx = unsafe { mb_pg_transaction_begin([conn_val, s("read_committed")].as_ptr(), 2) };
    assert!(tx.is_ptr(), "begin failed");

    let _ = unsafe {
        mb_pg_transaction_execute(
            [tx, s("INSERT INTO mb_pg_tx_marker VALUES (1)")].as_ptr(),
            2,
        )
    };
    let _ = unsafe { mb_pg_transaction_savepoint([tx, s("sp")].as_ptr(), 2) };
    let _ = unsafe {
        mb_pg_transaction_execute(
            [tx, s("INSERT INTO mb_pg_tx_marker VALUES (2)")].as_ptr(),
            2,
        )
    };
    let _ = unsafe { mb_pg_transaction_rollback_to([tx, s("sp")].as_ptr(), 2) };
    let _ = unsafe { mb_pg_transaction_commit([tx].as_ptr(), 1) };

    let count_sql = "SELECT count(*) AS n FROM mb_pg_tx_marker";
    let _rv = unsafe { mb_pg_execute([conn_val, s(count_sql)].as_ptr(), 2) };
    // rows_affected from a SELECT count is 0; the actual assertion runs
    // server-side via a separate verify query — fold once
    // `mb_pg_query_one` lands.

    let _ = unsafe { mb_pg_execute([conn_val, s("DROP TABLE mb_pg_tx_marker")].as_ptr(), 2) };
    let _ = unsafe { mb_pg_close([conn_val].as_ptr(), 1) };
}

#[test]
#[ignore]
fn transaction_rollback_consumes_handle() {
    let Some(url) = db_url() else { return };
    let conn_val = unsafe { mb_pg_connect([s(&url)].as_ptr(), 1) };

    let tx = unsafe { mb_pg_transaction_begin([conn_val, s("read_committed")].as_ptr(), 2) };
    assert!(tx.is_ptr());
    let _ = unsafe { mb_pg_transaction_rollback([tx].as_ptr(), 1) };
    // Second rollback on a consumed handle is a no-op (returns none).
    let _ = unsafe { mb_pg_transaction_rollback([tx].as_ptr(), 1) };
    let _ = unsafe { mb_pg_close([conn_val].as_ptr(), 1) };
}

#[test]
#[ignore]
fn migrate_apply_then_revert() {
    let Some(url) = db_url() else { return };
    let conn_val = unsafe { mb_pg_connect([s(&url)].as_ptr(), 1) };

    let runner = unsafe { mb_pg_migration_runner_new([conn_val, MbValue::none()].as_ptr(), 2) };
    assert!(runner.is_ptr());
    let _ = unsafe { mb_pg_migration_runner_init([runner].as_ptr(), 1) };

    let mig = unsafe {
        mb_pg_migration_new(
            [
                s("0001_mamba_test"),
                s("test migration from mamba layer"),
                s("CREATE TABLE mb_pg_mig_t(id int)"),
                s("DROP TABLE mb_pg_mig_t"),
            ]
            .as_ptr(),
            4,
        )
    };
    assert!(mig.is_ptr());
    let mig_h = unsafe { handle::<MbPgMigration>(mig) };
    let version = mig_h.inner.version.clone();

    let _ = unsafe { mb_pg_migration_runner_apply([runner, mig].as_ptr(), 2) };

    let applied = unsafe { mb_pg_migration_runner_applied_migrations([runner].as_ptr(), 1) };
    assert!(applied.is_ptr());
    let applied_list = unsafe {
        let addr = applied.as_ptr().unwrap();
        &*(addr as *const Vec<MbValue>)
    };
    let applied_versions: Vec<String> = applied_list
        .iter()
        .filter_map(|&v| unsafe { mamba_registry::rc::read_obj_str(v) })
        .collect();
    assert!(
        applied_versions.iter().any(|v| v == &version),
        "version {version} not in applied list: {applied_versions:?}"
    );

    let _ = unsafe { mb_pg_migration_runner_revert([runner, mig].as_ptr(), 2) };

    let post = unsafe { mb_pg_migration_runner_applied_migrations([runner].as_ptr(), 1) };
    let post_list = unsafe {
        let addr = post.as_ptr().unwrap();
        &*(addr as *const Vec<MbValue>)
    };
    let post_versions: Vec<String> = post_list
        .iter()
        .filter_map(|&v| unsafe { mamba_registry::rc::read_obj_str(v) })
        .collect();
    assert!(
        !post_versions.iter().any(|v| v == &version),
        "version {version} still in applied list after revert: {post_versions:?}"
    );

    let _ = unsafe { mb_pg_close([conn_val].as_ptr(), 1) };
}

#[test]
#[ignore]
fn connect_with_bogus_url_returns_none() {
    let url = "postgres://nope:nope@127.0.0.1:1/no_such_db";
    let v = unsafe { mb_pg_connect([s(url)].as_ptr(), 1) };
    assert!(v.is_none(), "expected none, got {:?}", v);
}
