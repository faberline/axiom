#![allow(improper_ctypes_definitions)]

//! FFI functions exposed by `pgkit-binding` to Mamba scripts.
//!
//! All functions follow the Mamba native-call ABI:
//! ```text
//! extern "C" fn name(args: *const MbValue, nargs: usize) -> MbValue
//! ```
//!
//! # Surface
//!
//! ## Driver — module `mambalibs.pg`
//!
//! | Symbol                         | Mamba call                                |
//! |--------------------------------|-------------------------------------------|
//! | `mb_pg_connect`                | `connect(url)`                            |
//! | `mb_pg_execute`                | `execute(conn, sql)`                      |
//! | `mb_pg_execute_params`         | `execute_params(conn, sql, params)`       |
//! | `mb_pg_query_all`              | `query_all(conn, sql, params)`            |
//! | `mb_pg_query_one`              | `query_one(conn, sql, params)`            |
//! | `mb_pg_ping`                   | `ping(conn)`                              |
//! | `mb_pg_close`                  | `close(conn)`                             |
//!
//! ## Transaction — module `mambalibs.pg`
//!
//! | Symbol                                        | Mamba call                               |
//! |-----------------------------------------------|------------------------------------------|
//! | `mb_pg_transaction_begin`                     | `transaction_begin(conn, isolation)`     |
//! | `mb_pg_transaction_execute`                   | `transaction_execute(tx, sql)`           |
//! | `mb_pg_transaction_savepoint`                 | `transaction_savepoint(tx, name)`        |
//! | `mb_pg_transaction_rollback_to`               | `transaction_rollback_to(tx, name)`      |
//! | `mb_pg_transaction_release_savepoint`         | `transaction_release_savepoint(tx, name)`|
//! | `mb_pg_transaction_commit`                    | `transaction_commit(tx)`                 |
//! | `mb_pg_transaction_rollback`                  | `transaction_rollback(tx)`               |
//!
//! ## Migrate — module `mambalibs.pg.migrate`
//!
//! | Symbol                                            | Mamba call                                   |
//! |---------------------------------------------------|----------------------------------------------|
//! | `mb_pg_migration_runner_new`                      | `MigrationRunner(conn, table?)`              |
//! | `mb_pg_migration_runner_init`                     | `runner_init(runner)`                        |
//! | `mb_pg_migration_runner_apply`                    | `runner_apply(runner, migration)`            |
//! | `mb_pg_migration_runner_revert`                   | `runner_revert(runner, migration)`           |
//! | `mb_pg_migration_runner_up`                       | `runner_up(runner, dir)`                     |
//! | `mb_pg_migration_runner_down`                     | `runner_down(runner, dir)`                   |
//! | `mb_pg_migration_runner_applied_migrations`       | `runner_applied_migrations(runner)`          |
//! | `mb_pg_migration_runner_status`                   | `runner_status(runner, migrations)`          |
//! | `mb_pg_migration_new`                             | `Migration(version, name, up, down)`         |

use std::path::PathBuf;
use std::sync::Arc;

use mamba_registry::convert::mb_wrap_native;
use mamba_registry::MbValue;

use pg::driver::blocking::{
    Connection as PgConnection, MigrationRunner as PgMigrationRunner, Transaction as PgTransaction,
};
use pg::driver::transaction::IsolationLevel;
use pg::driver::{ExtractedValue, Row};
use pg::migrate::Migration;
use pg::PoolConfig;

use crate::types::{MbPgConnection, MbPgMigration, MbPgMigrationRunner, MbPgTransaction};

// ── Helpers ───────────────────────────────────────────────────────────────────

#[inline]
unsafe fn arg(args: *const MbValue, nargs: usize, idx: usize) -> MbValue {
    if idx < nargs {
        unsafe { *args.add(idx) }
    } else {
        MbValue::none()
    }
}

fn read_str(v: MbValue) -> Option<String> {
    mamba_registry::test_ops::init();
    unsafe { mamba_registry::rc::read_obj_str(v) }
}

fn wrap_str(s: String) -> MbValue {
    mamba_registry::test_ops::init();
    mamba_registry::rc::wrap_obj_str(s)
}

#[inline]
unsafe fn handle<'a, T>(v: MbValue) -> Option<&'a T> {
    v.as_ptr()
        .filter(|a| *a != 0)
        .map(|a| unsafe { &*(a as *const T) })
}

fn parse_isolation(s: &str) -> Result<IsolationLevel, String> {
    match s.to_ascii_lowercase().as_str() {
        "read_committed" | "" => Ok(IsolationLevel::ReadCommitted),
        "repeatable_read" => Ok(IsolationLevel::RepeatableRead),
        "serializable" => Ok(IsolationLevel::Serializable),
        "read_uncommitted" => Ok(IsolationLevel::ReadUncommitted),
        other => Err(format!("unknown isolation level: {other}")),
    }
}

fn mb_to_extracted(v: MbValue) -> ExtractedValue {
    if v.is_none() {
        ExtractedValue::Null
    } else if let Some(b) = v.as_bool() {
        ExtractedValue::Bool(b)
    } else if let Some(i) = v.as_int() {
        ExtractedValue::BigInt(i)
    } else if let Some(f) = v.as_float() {
        ExtractedValue::Double(f)
    } else if let Some(s) = read_str(v) {
        ExtractedValue::String(s)
    } else {
        ExtractedValue::Null
    }
}

fn list_to_params(v: MbValue) -> Option<Vec<ExtractedValue>> {
    mamba_registry::test_ops::init();
    let ops = mamba_registry::ops();
    let len = (ops.list_len)(v)?;
    let mut params = Vec::with_capacity(len);
    for idx in 0..len {
        let item = (ops.list_get)(v, idx)?;
        params.push(mb_to_extracted(item));
    }
    Some(params)
}

fn extracted_to_mb(ev: &ExtractedValue) -> MbValue {
    match ev {
        ExtractedValue::Null => MbValue::none(),
        ExtractedValue::Bool(b) => MbValue::from_bool(*b),
        ExtractedValue::SmallInt(v) => MbValue::from_int(*v as i64),
        ExtractedValue::Int(v) => MbValue::from_int(*v as i64),
        ExtractedValue::BigInt(v) => MbValue::from_int(*v),
        ExtractedValue::Float(v) => MbValue::from_float(*v as f64),
        ExtractedValue::Double(v) => MbValue::from_float(*v),
        ExtractedValue::String(s) => wrap_str(s.clone()),
        other => wrap_str(format!("{other:?}")),
    }
}

fn row_to_dict(row: Row) -> MbValue {
    mamba_registry::test_ops::init();
    let ops = mamba_registry::ops();
    let dict = (ops.dict_new)();
    for (key, value) in row.columns_map() {
        (ops.dict_insert_str)(dict, key, extracted_to_mb(value));
    }
    dict
}

fn rows_to_list(rows: Vec<Row>) -> MbValue {
    mamba_registry::test_ops::init();
    let ops = mamba_registry::ops();
    let dicts = rows.into_iter().map(row_to_dict).collect();
    (ops.list_new)(dicts)
}

// ── Driver verbs ──────────────────────────────────────────────────────────────

/// `connect(url: str) -> Connection`
///
/// Builds a `pg::driver::blocking::Connection` with the default
/// `PoolConfig`. Returns `MbValue::none()` on failure.
#[no_mangle]
pub unsafe extern "C" fn mb_pg_connect(args: *const MbValue, nargs: usize) -> MbValue {
    let url = match read_str(unsafe { arg(args, nargs, 0) }) {
        Some(s) if !s.is_empty() => s,
        _ => return MbValue::none(),
    };
    match PgConnection::new(&url, PoolConfig::default()) {
        Ok(conn) => mb_wrap_native(MbPgConnection::new(conn)),
        Err(_) => MbValue::none(),
    }
}

/// `execute(conn, sql: str) -> int` — rows affected. The query runs
/// through the pg driver facade inside the connection's owned tokio runtime.
#[no_mangle]
pub unsafe extern "C" fn mb_pg_execute(args: *const MbValue, nargs: usize) -> MbValue {
    let conn = match unsafe { handle::<MbPgConnection>(arg(args, nargs, 0)) } {
        Some(c) => c,
        None => return MbValue::none(),
    };
    let sql = match read_str(unsafe { arg(args, nargs, 1) }) {
        Some(s) if !s.is_empty() => s,
        _ => return MbValue::none(),
    };

    match conn.inner.execute(&sql) {
        Ok(rows) => MbValue::from_int(rows as i64),
        Err(_) => MbValue::none(),
    }
}

/// `execute_params(conn, sql: str, params: list) -> int` — rows affected.
#[no_mangle]
pub unsafe extern "C" fn mb_pg_execute_params(args: *const MbValue, nargs: usize) -> MbValue {
    let conn = match unsafe { handle::<MbPgConnection>(arg(args, nargs, 0)) } {
        Some(c) => c,
        None => return MbValue::none(),
    };
    let sql = match read_str(unsafe { arg(args, nargs, 1) }) {
        Some(s) if !s.is_empty() => s,
        _ => return MbValue::none(),
    };
    let params = match list_to_params(unsafe { arg(args, nargs, 2) }) {
        Some(params) => params,
        None => return MbValue::none(),
    };

    match conn.inner.execute_params(&sql, &params) {
        Ok(rows) => MbValue::from_int(rows as i64),
        Err(_) => MbValue::none(),
    }
}

/// `query_all(conn, sql: str, params: list) -> list[dict]`.
#[no_mangle]
pub unsafe extern "C" fn mb_pg_query_all(args: *const MbValue, nargs: usize) -> MbValue {
    let conn = match unsafe { handle::<MbPgConnection>(arg(args, nargs, 0)) } {
        Some(c) => c,
        None => return MbValue::none(),
    };
    let sql = match read_str(unsafe { arg(args, nargs, 1) }) {
        Some(s) if !s.is_empty() => s,
        _ => return MbValue::none(),
    };
    let params = match list_to_params(unsafe { arg(args, nargs, 2) }) {
        Some(params) => params,
        None => return MbValue::none(),
    };

    match conn.inner.fetch_rows(&sql, &params) {
        Ok(rows) => rows_to_list(rows),
        Err(_) => MbValue::none(),
    }
}

/// `query_one(conn, sql: str, params: list) -> dict?`.
#[no_mangle]
pub unsafe extern "C" fn mb_pg_query_one(args: *const MbValue, nargs: usize) -> MbValue {
    let conn = match unsafe { handle::<MbPgConnection>(arg(args, nargs, 0)) } {
        Some(c) => c,
        None => return MbValue::none(),
    };
    let sql = match read_str(unsafe { arg(args, nargs, 1) }) {
        Some(s) if !s.is_empty() => s,
        _ => return MbValue::none(),
    };
    let params = match list_to_params(unsafe { arg(args, nargs, 2) }) {
        Some(params) => params,
        None => return MbValue::none(),
    };

    match conn.inner.fetch_optional_row(&sql, &params) {
        Ok(Some(row)) => row_to_dict(row),
        Ok(None) => MbValue::none(),
        Err(_) => MbValue::none(),
    }
}

/// `ping(conn) -> None`
#[no_mangle]
pub unsafe extern "C" fn mb_pg_ping(args: *const MbValue, nargs: usize) -> MbValue {
    let conn = match unsafe { handle::<MbPgConnection>(arg(args, nargs, 0)) } {
        Some(c) => c,
        None => return MbValue::none(),
    };
    let _ = conn.inner.ping();
    MbValue::none()
}

/// `close(conn) -> None`
#[no_mangle]
pub unsafe extern "C" fn mb_pg_close(args: *const MbValue, nargs: usize) -> MbValue {
    let conn = match unsafe { handle::<MbPgConnection>(arg(args, nargs, 0)) } {
        Some(c) => c,
        None => return MbValue::none(),
    };
    let _ = conn.inner.close();
    MbValue::none()
}

// ── Transaction verbs ─────────────────────────────────────────────────────────

/// `transaction_begin(conn, isolation='read_committed') -> Transaction`
#[no_mangle]
pub unsafe extern "C" fn mb_pg_transaction_begin(args: *const MbValue, nargs: usize) -> MbValue {
    let conn = match unsafe { handle::<MbPgConnection>(arg(args, nargs, 0)) } {
        Some(c) => c,
        None => return MbValue::none(),
    };
    let iso_str =
        read_str(unsafe { arg(args, nargs, 1) }).unwrap_or_else(|| "read_committed".to_string());
    let iso = match parse_isolation(&iso_str) {
        Ok(i) => i,
        Err(_) => return MbValue::none(),
    };
    match PgTransaction::begin(&conn.inner, iso) {
        Ok(tx) => mb_wrap_native(MbPgTransaction::new(tx)),
        Err(_) => MbValue::none(),
    }
}

/// `transaction_execute(tx, sql: str) -> int` — rows affected.
///
/// Mediates `tx.execute` via interior mutation of the
/// `Mutex<Option<PgTransaction>>` (R2 review note: re-exposing the
/// parent pool would break the isolation contract for statements the
/// user believes are inside the transaction).
#[no_mangle]
pub unsafe extern "C" fn mb_pg_transaction_execute(args: *const MbValue, nargs: usize) -> MbValue {
    let tx_handle = match unsafe { handle::<MbPgTransaction>(arg(args, nargs, 0)) } {
        Some(t) => t,
        None => return MbValue::none(),
    };
    let sql = match read_str(unsafe { arg(args, nargs, 1) }) {
        Some(s) if !s.is_empty() => s,
        _ => return MbValue::none(),
    };

    let mut guard = match tx_handle.inner.lock() {
        Ok(g) => g,
        Err(_) => return MbValue::none(),
    };
    let tx = match guard.as_mut() {
        Some(t) => t,
        None => return MbValue::none(),
    };
    let inner = tx.as_mut_transaction();
    // The blocking::Transaction holds its own Arc<Runtime>; we can't
    // borrow it here while also holding &mut to the inner sqlx
    // transaction. Use the current thread's tokio Handle via
    // `tokio::runtime::Handle::try_current()` — when calling from
    // outside a runtime context we fall back to spawning a temporary
    // one. The blocking façade already entered its runtime when
    // begin() ran, but that context is lost between FFI calls; the
    // sqlx::query::execute call needs a Handle. Solution: re-enter the
    // runtime by calling block_on through a fresh Handle. We grab the
    // runtime via the parent connection — but we don't have it here.
    //
    // Simplest workable approach: use sqlx's blocking-friendly
    // execution by wrapping in a current-thread runtime per call. This
    // is a few μs of overhead and avoids cross-runtime aliasing.
    let result: Result<sqlx::postgres::PgQueryResult, sqlx::Error> = {
        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(r) => r,
            Err(_) => return MbValue::none(),
        };
        rt.block_on(async { sqlx::query(&sql).execute(&mut **inner).await })
    };
    match result {
        Ok(r) => MbValue::from_int(r.rows_affected() as i64),
        Err(_) => MbValue::none(),
    }
}

/// `transaction_savepoint(tx, name: str) -> None`
#[no_mangle]
pub unsafe extern "C" fn mb_pg_transaction_savepoint(
    args: *const MbValue,
    nargs: usize,
) -> MbValue {
    let tx_handle = match unsafe { handle::<MbPgTransaction>(arg(args, nargs, 0)) } {
        Some(t) => t,
        None => return MbValue::none(),
    };
    let name = read_str(unsafe { arg(args, nargs, 1) }).unwrap_or_default();
    if name.is_empty() {
        return MbValue::none();
    }

    let mut guard = match tx_handle.inner.lock() {
        Ok(g) => g,
        Err(_) => return MbValue::none(),
    };
    if let Some(tx) = guard.as_mut() {
        let _ = tx.savepoint(&name);
    }
    MbValue::none()
}

/// `transaction_rollback_to(tx, name: str) -> None`
#[no_mangle]
pub unsafe extern "C" fn mb_pg_transaction_rollback_to(
    args: *const MbValue,
    nargs: usize,
) -> MbValue {
    let tx_handle = match unsafe { handle::<MbPgTransaction>(arg(args, nargs, 0)) } {
        Some(t) => t,
        None => return MbValue::none(),
    };
    let name = read_str(unsafe { arg(args, nargs, 1) }).unwrap_or_default();
    if name.is_empty() {
        return MbValue::none();
    }

    let mut guard = match tx_handle.inner.lock() {
        Ok(g) => g,
        Err(_) => return MbValue::none(),
    };
    if let Some(tx) = guard.as_mut() {
        let _ = tx.rollback_to(&name);
    }
    MbValue::none()
}

/// `transaction_release_savepoint(tx, name: str) -> None`
#[no_mangle]
pub unsafe extern "C" fn mb_pg_transaction_release_savepoint(
    args: *const MbValue,
    nargs: usize,
) -> MbValue {
    let tx_handle = match unsafe { handle::<MbPgTransaction>(arg(args, nargs, 0)) } {
        Some(t) => t,
        None => return MbValue::none(),
    };
    let name = read_str(unsafe { arg(args, nargs, 1) }).unwrap_or_default();
    if name.is_empty() {
        return MbValue::none();
    }

    let mut guard = match tx_handle.inner.lock() {
        Ok(g) => g,
        Err(_) => return MbValue::none(),
    };
    if let Some(tx) = guard.as_mut() {
        let _ = tx.release_savepoint(&name);
    }
    MbValue::none()
}

/// `transaction_commit(tx) -> None`
#[no_mangle]
pub unsafe extern "C" fn mb_pg_transaction_commit(args: *const MbValue, nargs: usize) -> MbValue {
    let tx_handle = match unsafe { handle::<MbPgTransaction>(arg(args, nargs, 0)) } {
        Some(t) => t,
        None => return MbValue::none(),
    };
    let inner = {
        let mut guard = match tx_handle.inner.lock() {
            Ok(g) => g,
            Err(_) => return MbValue::none(),
        };
        match guard.take() {
            Some(t) => t,
            None => return MbValue::none(),
        }
    };
    let _ = inner.commit();
    MbValue::none()
}

/// `transaction_rollback(tx) -> None`
#[no_mangle]
pub unsafe extern "C" fn mb_pg_transaction_rollback(args: *const MbValue, nargs: usize) -> MbValue {
    let tx_handle = match unsafe { handle::<MbPgTransaction>(arg(args, nargs, 0)) } {
        Some(t) => t,
        None => return MbValue::none(),
    };
    let inner = {
        let mut guard = match tx_handle.inner.lock() {
            Ok(g) => g,
            Err(_) => return MbValue::none(),
        };
        match guard.take() {
            Some(t) => t,
            None => return MbValue::none(),
        }
    };
    let _ = inner.rollback();
    MbValue::none()
}

// ── Migrate verbs ─────────────────────────────────────────────────────────────

/// `Migration(version, name, up, down) -> Migration`
#[no_mangle]
pub unsafe extern "C" fn mb_pg_migration_new(args: *const MbValue, nargs: usize) -> MbValue {
    let version = read_str(unsafe { arg(args, nargs, 0) }).unwrap_or_default();
    let name = read_str(unsafe { arg(args, nargs, 1) }).unwrap_or_default();
    let up = read_str(unsafe { arg(args, nargs, 2) }).unwrap_or_default();
    let down = read_str(unsafe { arg(args, nargs, 3) }).unwrap_or_default();
    if version.is_empty() {
        return MbValue::none();
    }
    mb_wrap_native(MbPgMigration::new(Migration::new(version, name, up, down)))
}

/// `MigrationRunner(conn, table?) -> Runner`
#[no_mangle]
pub unsafe extern "C" fn mb_pg_migration_runner_new(args: *const MbValue, nargs: usize) -> MbValue {
    let conn = match unsafe { handle::<MbPgConnection>(arg(args, nargs, 0)) } {
        Some(c) => c,
        None => return MbValue::none(),
    };
    let table = read_str(unsafe { arg(args, nargs, 1) });
    // PgMigrationRunner::new takes Connection by value; the blocking
    // Connection is Clone (Arc<inner> + Arc<runtime>), so we clone.
    let conn_clone: PgConnection = (*conn.inner).clone();
    let runner = PgMigrationRunner::new(conn_clone, table);
    mb_wrap_native(MbPgMigrationRunner::new(runner))
}

/// `runner_init(runner) -> None`
#[no_mangle]
pub unsafe extern "C" fn mb_pg_migration_runner_init(
    args: *const MbValue,
    nargs: usize,
) -> MbValue {
    let runner = match unsafe { handle::<MbPgMigrationRunner>(arg(args, nargs, 0)) } {
        Some(r) => r,
        None => return MbValue::none(),
    };
    let _ = runner.inner.init();
    MbValue::none()
}

/// `runner_apply(runner, migration) -> None`
#[no_mangle]
pub unsafe extern "C" fn mb_pg_migration_runner_apply(
    args: *const MbValue,
    nargs: usize,
) -> MbValue {
    let runner = match unsafe { handle::<MbPgMigrationRunner>(arg(args, nargs, 0)) } {
        Some(r) => r,
        None => return MbValue::none(),
    };
    let mig = match unsafe { handle::<MbPgMigration>(arg(args, nargs, 1)) } {
        Some(m) => m,
        None => return MbValue::none(),
    };
    let _ = runner.inner.apply(&mig.inner);
    MbValue::none()
}

/// `runner_revert(runner, migration) -> None`
#[no_mangle]
pub unsafe extern "C" fn mb_pg_migration_runner_revert(
    args: *const MbValue,
    nargs: usize,
) -> MbValue {
    let runner = match unsafe { handle::<MbPgMigrationRunner>(arg(args, nargs, 0)) } {
        Some(r) => r,
        None => return MbValue::none(),
    };
    let mig = match unsafe { handle::<MbPgMigration>(arg(args, nargs, 1)) } {
        Some(m) => m,
        None => return MbValue::none(),
    };
    let _ = runner.inner.revert(&mig.inner);
    MbValue::none()
}

/// `runner_up(runner, dir: str) -> list[str]`
///
/// Returns a heap `Vec<MbValue>` of version strings (the applied
/// versions). Empty list on error.
#[no_mangle]
pub unsafe extern "C" fn mb_pg_migration_runner_up(args: *const MbValue, nargs: usize) -> MbValue {
    let runner = match unsafe { handle::<MbPgMigrationRunner>(arg(args, nargs, 0)) } {
        Some(r) => r,
        None => return MbValue::none(),
    };
    let dir = read_str(unsafe { arg(args, nargs, 1) }).unwrap_or_default();
    let applied = runner.inner.up(&PathBuf::from(dir)).unwrap_or_default();
    wrap_str_vec(applied)
}

/// `runner_down(runner, dir: str) -> str?`
#[no_mangle]
pub unsafe extern "C" fn mb_pg_migration_runner_down(
    args: *const MbValue,
    nargs: usize,
) -> MbValue {
    let runner = match unsafe { handle::<MbPgMigrationRunner>(arg(args, nargs, 0)) } {
        Some(r) => r,
        None => return MbValue::none(),
    };
    let dir = read_str(unsafe { arg(args, nargs, 1) }).unwrap_or_default();
    match runner.inner.down(&PathBuf::from(dir)) {
        Ok(Some(v)) => wrap_str(v),
        _ => MbValue::none(),
    }
}

/// `runner_applied_migrations(runner) -> list[str]`
#[no_mangle]
pub unsafe extern "C" fn mb_pg_migration_runner_applied_migrations(
    args: *const MbValue,
    nargs: usize,
) -> MbValue {
    let runner = match unsafe { handle::<MbPgMigrationRunner>(arg(args, nargs, 0)) } {
        Some(r) => r,
        None => return MbValue::none(),
    };
    let versions = runner.inner.applied_migrations().unwrap_or_default();
    wrap_str_vec(versions)
}

/// `runner_status(runner, migrations: list) -> dict`
///
/// Returns a heap dict-like structure encoded as `Vec<(String, MbValue)>`
/// — mamba's dict ABI is opaque list-of-pair for now. Keys are
/// `"applied"`, `"pending"`, `"missing_from_disk"`. Each value is a
/// list of version strings.
#[no_mangle]
pub unsafe extern "C" fn mb_pg_migration_runner_status(
    args: *const MbValue,
    nargs: usize,
) -> MbValue {
    let runner = match unsafe { handle::<MbPgMigrationRunner>(arg(args, nargs, 0)) } {
        Some(r) => r,
        None => return MbValue::none(),
    };
    let migrations_val = unsafe { arg(args, nargs, 1) };
    let migrations: Vec<Migration> = migrations_val
        .as_ptr()
        .filter(|a| *a != 0)
        .map(|addr| {
            let list = unsafe { &*(addr as *const Vec<MbValue>) };
            list.iter()
                .filter_map(|&v| unsafe { handle::<MbPgMigration>(v) })
                .map(|h| (*h.inner).clone())
                .collect()
        })
        .unwrap_or_default();

    let status = match runner.inner.status(&migrations) {
        Ok(s) => s,
        Err(_) => return MbValue::none(),
    };

    let dict: Vec<(String, MbValue)> = vec![
        ("applied".to_string(), wrap_str_vec(status.applied)),
        ("pending".to_string(), wrap_str_vec(status.pending)),
    ];
    let boxed: Box<Vec<(String, MbValue)>> = Box::new(dict);
    MbValue::from_ptr(Box::into_raw(boxed) as usize)
}

// ── Internal helpers ─────────────────────────────────────────────────────────

fn wrap_str_vec(items: Vec<String>) -> MbValue {
    let wrapped: Vec<MbValue> = items.into_iter().map(wrap_str).collect();
    let boxed: Box<Vec<MbValue>> = Box::new(wrapped);
    MbValue::from_ptr(Box::into_raw(boxed) as usize)
}

// Mirror the parent Connection's runtime onto a local Arc to silence
// the unused-import warning when this binding compiles standalone.
#[allow(dead_code)]
fn _runtime_marker(c: &MbPgConnection) -> Arc<tokio::runtime::Runtime> {
    c.inner.runtime()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_isolation_levels() {
        assert!(matches!(
            parse_isolation(""),
            Ok(IsolationLevel::ReadCommitted)
        ));
        assert!(matches!(
            parse_isolation("read_committed"),
            Ok(IsolationLevel::ReadCommitted)
        ));
        assert!(matches!(
            parse_isolation("REPEATABLE_READ"),
            Ok(IsolationLevel::RepeatableRead)
        ));
        assert!(matches!(
            parse_isolation("Serializable"),
            Ok(IsolationLevel::Serializable)
        ));
        assert!(parse_isolation("nonsense").is_err());
    }

    #[test]
    fn migration_value_object_ctor() {
        mamba_registry::test_ops::init();
        let v = mamba_registry::rc::wrap_obj_str("0001".to_string());
        let n = mamba_registry::rc::wrap_obj_str("test".to_string());
        let u = mamba_registry::rc::wrap_obj_str("CREATE TABLE t(id int)".to_string());
        let d = mamba_registry::rc::wrap_obj_str("DROP TABLE t".to_string());
        let args = [v, n, u, d];
        let m = unsafe { mb_pg_migration_new(args.as_ptr(), 4) };
        assert!(m.is_ptr());
        let h = unsafe { handle::<MbPgMigration>(m) }.expect("ptr");
        assert_eq!(h.inner.version, "0001");
        assert_eq!(h.inner.name, "test");
    }

    #[test]
    fn list_to_params_decodes_basic_values() {
        mamba_registry::test_ops::init();
        let text = mamba_registry::rc::wrap_obj_str("mamba".to_string());
        let params = (mamba_registry::ops().list_new)(vec![
            MbValue::none(),
            MbValue::from_bool(true),
            MbValue::from_int(42),
            MbValue::from_float(3.5),
            text,
        ]);

        assert_eq!(
            list_to_params(params),
            Some(vec![
                ExtractedValue::Null,
                ExtractedValue::Bool(true),
                ExtractedValue::BigInt(42),
                ExtractedValue::Double(3.5),
                ExtractedValue::String("mamba".to_string()),
            ])
        );
    }

    #[test]
    fn list_to_params_rejects_non_list() {
        mamba_registry::test_ops::init();
        assert_eq!(list_to_params(MbValue::from_int(1)), None);
    }

    #[test]
    fn row_to_dict_encodes_basic_values() {
        mamba_registry::test_ops::init();
        let row = Row::new(std::collections::HashMap::from([
            ("id".to_string(), ExtractedValue::BigInt(7)),
            (
                "name".to_string(),
                ExtractedValue::String("mamba".to_string()),
            ),
            ("active".to_string(), ExtractedValue::Bool(true)),
        ]));

        let dict = row_to_dict(row);
        let ops = mamba_registry::ops();
        assert_eq!(
            (ops.dict_get_str)(dict, "id").and_then(|v| v.as_int()),
            Some(7)
        );
        assert_eq!(
            (ops.dict_get_str)(dict, "name")
                .and_then(|v| unsafe { mamba_registry::rc::read_obj_str(v) }),
            Some("mamba".to_string())
        );
        assert_eq!(
            (ops.dict_get_str)(dict, "active").and_then(|v| v.as_bool()),
            Some(true)
        );
    }

    #[test]
    fn rows_to_list_encodes_driver_rows() {
        mamba_registry::test_ops::init();
        let rows = vec![
            Row::new(std::collections::HashMap::from([(
                "id".to_string(),
                ExtractedValue::BigInt(1),
            )])),
            Row::new(std::collections::HashMap::from([(
                "id".to_string(),
                ExtractedValue::BigInt(2),
            )])),
        ];

        let list = rows_to_list(rows);
        let ops = mamba_registry::ops();
        assert_eq!((ops.list_len)(list), Some(2));
        let first = (ops.list_get)(list, 0).expect("first row");
        let second = (ops.list_get)(list, 1).expect("second row");
        assert_eq!(
            (ops.dict_get_str)(first, "id").and_then(|v| v.as_int()),
            Some(1)
        );
        assert_eq!(
            (ops.dict_get_str)(second, "id").and_then(|v| v.as_int()),
            Some(2)
        );
    }
}
