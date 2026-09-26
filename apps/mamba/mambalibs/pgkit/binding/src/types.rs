//! Opaque handle types for the `pgkit-binding` FFI layer.
//!
//! Each handle wraps the corresponding `pg::driver::blocking`
//! façade type (plus the orm `Session` surface):
//!
//! | Handle                  | Wraps                                                            |
//! |-------------------------|------------------------------------------------------------------|
//! | [`MbPgConnection`]      | `Arc<pg::driver::blocking::Connection>`                    |
//! | [`MbPgTransaction`]     | `Mutex<Option<pg::driver::blocking::Transaction>>`         |
//! | [`MbPgMigrationRunner`] | `Arc<pg::driver::blocking::MigrationRunner>`               |
//! | [`MbPgMigration`]       | `Arc<pg::migrate::Migration>`                              |
//! | [`MbPgSession`]         | `Mutex<Option<OwnedSession>>` (self-referential Session+Conn)    |

// HANDWRITE-BEGIN reason: mamba-FFI generator codegen gap; no
//   section type today emits MbValue ABI wrappers around a typed
//   Rust API. Closes when score grows a `mamba-binding` section
//   type.
//! @spec
//!   .aw/tech-design/projects/pg/specs/pg-orm-session-uow.md#changes

use std::sync::{Arc, Mutex};

use pg::blocking::Session as PgSession;
use pg::driver::blocking::{
    Connection as PgConnection, MigrationRunner as PgMigrationRunner, Transaction as PgTransaction,
};
use pg::migrate::Migration;

/// Mamba handle for a pooled Postgres connection.
///
/// Holds an `Arc` to the blocking façade `Connection`, which owns the
/// `PgPool` + `Arc<tokio::runtime::Runtime>`. Cloning the handle clones
/// the `Arc` — the pool is shared.
pub struct MbPgConnection {
    pub inner: std::sync::Arc<PgConnection>,
}

impl MbPgConnection {
    pub fn new(inner: PgConnection) -> Self {
        Self {
            inner: std::sync::Arc::new(inner),
        }
    }
}

/// Mamba handle for an in-flight transaction.
///
/// `commit` / `rollback` on the blocking façade consume the inner
/// `Transaction` by value, so we keep it inside `Mutex<Option<_>>` and
/// `take()` on terminal verbs. Subsequent operations on a consumed
/// handle return an error.
pub struct MbPgTransaction {
    pub inner: Mutex<Option<PgTransaction>>,
}

impl MbPgTransaction {
    pub fn new(tx: PgTransaction) -> Self {
        Self {
            inner: Mutex::new(Some(tx)),
        }
    }
}

/// Mamba handle for a migration runner.
pub struct MbPgMigrationRunner {
    pub inner: std::sync::Arc<PgMigrationRunner>,
}

impl MbPgMigrationRunner {
    pub fn new(runner: PgMigrationRunner) -> Self {
        Self {
            inner: std::sync::Arc::new(runner),
        }
    }
}

/// Mamba handle for a `Migration` value object — pure data (version,
/// description, up_sql, down_sql), no IO.
pub struct MbPgMigration {
    pub inner: std::sync::Arc<Migration>,
}

impl MbPgMigration {
    pub fn new(m: Migration) -> Self {
        Self {
            inner: std::sync::Arc::new(m),
        }
    }
}

/// Self-referential wrapper: holds both an `Arc<PgConnection>` and a
/// `Session` that borrows from it.
///
/// Safety: `inner` is dropped before `_conn` (struct fields drop in
/// declaration order). While alive, `inner` holds a `&'static
/// PgConnection` reference produced by transmuting a borrow of `_conn`
/// — but since `_conn` is an `Arc` we own for the lifetime of this
/// wrapper, the underlying `PgConnection` lives at least as long as
/// `inner`. No outside code may mutate or drop the `PgConnection`
/// through this `Arc` because the only handle on it is the inner
/// session's borrow.
pub struct OwnedSession {
    inner: PgSession<'static>,
    _conn: Arc<PgConnection>,
}

impl OwnedSession {
    pub fn new(conn: Arc<PgConnection>) -> Self {
        let conn_ref: &PgConnection = &*conn;
        // SAFETY: see struct doc — `_conn` outlives `inner`.
        let conn_static: &'static PgConnection = unsafe { std::mem::transmute(conn_ref) };
        let inner = PgSession::new(conn_static);
        Self { inner, _conn: conn }
    }

    pub fn session(&mut self) -> &mut PgSession<'static> {
        &mut self.inner
    }
}

/// Mamba handle for an ORM Session.
///
/// `commit` / `rollback` / `close` consume the inner session, so we
/// keep it inside `Mutex<Option<_>>` and `take()` on terminal verbs —
/// same pattern as [`MbPgTransaction`].
pub struct MbPgSession {
    pub inner: Mutex<Option<OwnedSession>>,
}

impl MbPgSession {
    pub fn new(conn: Arc<PgConnection>) -> Self {
        Self {
            inner: Mutex::new(Some(OwnedSession::new(conn))),
        }
    }
}

// HANDWRITE-END
