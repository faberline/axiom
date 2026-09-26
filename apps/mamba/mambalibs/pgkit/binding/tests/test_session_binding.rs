//! Mamba-binding integration tests for the ORM Session FFI verbs.
//!
//! Each test bootstraps a `mb_session_users` table on a Postgres
//! pointed to by `$DATABASE_URL`, exercises the binding through the
//! native-call shim, and drops the table on teardown. Tests are
//! gated on `DATABASE_URL` — they skip silently when unset.
//!
//! Run with:
//!
//! ```text
//! DATABASE_URL=postgres://localhost/test cargo test \
//!     -p pgkit-binding --test test_session_binding -- --ignored
//! ```
//!
//! @spec
//!   .aw/tech-design/projects/pg/specs/pg-orm-session-uow.md#changes

// HANDWRITE-BEGIN reason: integration tests against a real Postgres are
//   inherently hand-written until codegen learns a
//   `test-plan-integration` section type.

#![allow(improper_ctypes_definitions)]

use mamba_registry::MbValue;

use pgkit_binding::methods::{mb_pg_close, mb_pg_connect, mb_pg_execute};
use pgkit_binding::session::{
    mb_pg_session_add, mb_pg_session_close, mb_pg_session_commit, mb_pg_session_get,
    mb_pg_session_new, mb_pg_session_query_all, mb_pg_session_rollback, mb_pg_session_slot_read,
    MbPgInsertSlot,
};

fn db_url() -> Option<String> {
    std::env::var("DATABASE_URL").ok().filter(|s| !s.is_empty())
}

fn s(v: &str) -> MbValue {
    mamba_registry::test_ops::init();
    mamba_registry::rc::wrap_obj_str(v.to_string())
}

unsafe fn handle<'a, T>(v: MbValue) -> &'a T {
    let addr = v.as_ptr().expect("ptr");
    unsafe { &*(addr as *const T) }
}

fn dict(pairs: &[(&str, MbValue)]) -> MbValue {
    mamba_registry::test_ops::init();
    let ops = mamba_registry::ops();
    let d = (ops.dict_new)();
    for (k, v) in pairs {
        (ops.dict_insert_str)(d, k, *v);
    }
    d
}

fn empty_dict() -> MbValue {
    mamba_registry::test_ops::init();
    let ops = mamba_registry::ops();
    (ops.dict_new)()
}

fn dict_get(v: MbValue, key: &str) -> Option<MbValue> {
    mamba_registry::test_ops::init();
    let ops = mamba_registry::ops();
    (ops.dict_get_str)(v, key)
}

fn bootstrap(conn: MbValue) {
    unsafe {
        let _ = mb_pg_execute(
            [conn, s("DROP TABLE IF EXISTS mb_session_users CASCADE")].as_ptr(),
            2,
        );
        let _ = mb_pg_execute(
            [
                conn,
                s("CREATE TABLE mb_session_users (
                    id BIGSERIAL PRIMARY KEY,
                    name TEXT NOT NULL,
                    email TEXT NOT NULL
                )"),
            ]
            .as_ptr(),
            2,
        );
    }
}

fn teardown(conn: MbValue) {
    unsafe {
        let _ = mb_pg_execute(
            [conn, s("DROP TABLE IF EXISTS mb_session_users CASCADE")].as_ptr(),
            2,
        );
        let _ = mb_pg_close([conn].as_ptr(), 1);
    }
}

#[test]
#[ignore]
fn session_lifecycle_new_close() {
    let Some(url) = db_url() else { return };
    let conn = unsafe { mb_pg_connect([s(&url)].as_ptr(), 1) };
    assert!(conn.is_ptr(), "connect failed");

    let sess = unsafe { mb_pg_session_new([conn].as_ptr(), 1) };
    assert!(sess.is_ptr(), "session_new failed");

    let _ = unsafe { mb_pg_session_close([sess].as_ptr(), 1) };
    // Second close on a consumed session is a no-op.
    let _ = unsafe { mb_pg_session_close([sess].as_ptr(), 1) };

    let _ = unsafe { mb_pg_close([conn].as_ptr(), 1) };
}

#[test]
#[ignore]
fn session_add_commit_then_get_round_trips() {
    let Some(url) = db_url() else { return };
    let conn = unsafe { mb_pg_connect([s(&url)].as_ptr(), 1) };
    bootstrap(conn);

    let sess = unsafe { mb_pg_session_new([conn].as_ptr(), 1) };
    let table = s("mb_session_users");
    let row = dict(&[("name", s("Alice")), ("email", s("a@e"))]);

    let slot = unsafe { mb_pg_session_add([sess, table, row].as_ptr(), 3) };
    assert!(slot.is_ptr(), "add did not return a slot handle");

    let _ = unsafe { mb_pg_session_commit([sess].as_ptr(), 1) };

    let slot_h = unsafe { handle::<MbPgInsertSlot>(slot) };
    let pk_after = *slot_h.inner.lock().unwrap();
    assert!(pk_after > 0, "slot pk was not populated after commit");

    let pk_int = unsafe { mb_pg_session_slot_read([slot].as_ptr(), 1) };
    assert_eq!(pk_int.as_int(), Some(pk_after));

    let got = unsafe { mb_pg_session_get([sess, table, MbValue::from_int(pk_after)].as_ptr(), 3) };
    assert!(got.is_ptr(), "get returned None for committed row");

    let name = dict_get(got, "name").expect("name key");
    let name_s = unsafe { mamba_registry::rc::read_obj_str(name) }.expect("name str");
    assert_eq!(name_s, "Alice");

    let _ = unsafe { mb_pg_session_close([sess].as_ptr(), 1) };
    teardown(conn);
}

#[test]
#[ignore]
fn session_rollback_discards_staging_and_clears_identity_map() {
    let Some(url) = db_url() else { return };
    let conn = unsafe { mb_pg_connect([s(&url)].as_ptr(), 1) };
    bootstrap(conn);

    let sess = unsafe { mb_pg_session_new([conn].as_ptr(), 1) };

    // Seed one row directly so the session has something to load.
    let _ = unsafe {
        mb_pg_execute(
            [
                conn,
                s("INSERT INTO mb_session_users (name, email) VALUES ('seed', 's@e')"),
            ]
            .as_ptr(),
            2,
        )
    };

    let table = s("mb_session_users");
    let row = dict(&[("name", s("tx_only")), ("email", s("t@e"))]);
    let _slot = unsafe { mb_pg_session_add([sess, table, row].as_ptr(), 3) };

    let _ = unsafe { mb_pg_session_rollback([sess].as_ptr(), 1) };

    // After rollback, the staged INSERT must not have landed.
    let all = unsafe { mb_pg_session_query_all([sess, table, empty_dict()].as_ptr(), 3) };
    assert!(all.is_ptr(), "query_all returned None");
    let list = unsafe {
        let addr = all.as_ptr().unwrap();
        &*(addr as *const Vec<MbValue>)
    };
    assert_eq!(
        list.len(),
        1,
        "rollback should have discarded staged INSERT; saw {} rows",
        list.len(),
    );

    let _ = unsafe { mb_pg_session_close([sess].as_ptr(), 1) };
    teardown(conn);
}

#[test]
#[ignore]
fn session_query_all_returns_dicts() {
    let Some(url) = db_url() else { return };
    let conn = unsafe { mb_pg_connect([s(&url)].as_ptr(), 1) };
    bootstrap(conn);

    let _ = unsafe {
        mb_pg_execute(
            [
                conn,
                s("INSERT INTO mb_session_users (name, email) VALUES
                       ('Alice', 'a@e'),
                       ('Bob', 'b@e')"),
            ]
            .as_ptr(),
            2,
        )
    };

    let sess = unsafe { mb_pg_session_new([conn].as_ptr(), 1) };
    let table = s("mb_session_users");
    let filter = dict(&[("name", s("Alice"))]);

    let all = unsafe { mb_pg_session_query_all([sess, table, filter].as_ptr(), 3) };
    assert!(all.is_ptr());

    let list = unsafe {
        let addr = all.as_ptr().unwrap();
        &*(addr as *const Vec<MbValue>)
    };
    assert_eq!(list.len(), 1, "filter must select exactly one row");

    let first = list[0];
    let name = dict_get(first, "name").expect("name key");
    let name_s = unsafe { mamba_registry::rc::read_obj_str(name) }.expect("name str");
    assert_eq!(name_s, "Alice");

    let _ = unsafe { mb_pg_session_close([sess].as_ptr(), 1) };
    teardown(conn);
}

// HANDWRITE-END
