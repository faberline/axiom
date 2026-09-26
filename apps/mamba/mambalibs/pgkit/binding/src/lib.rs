//! Mamba interface for the native `pg` PostgreSQL toolkit.
//!
//! Exposes PostgreSQL driver + migrate surface to Mamba scripts via the
//! `mamba-registry` infrastructure. ORM-shaped symbols
//! (DeclarativeBase, mapped_column, relationship, QueryBuilder, …) are
//! intentionally not mounted — they will return once
//! the `Session` API in pg's `orm` layer exists.
//!
//! # Module names
//!
//! - `mambalibs.pg`          → driver verbs + Transaction surface (this file)
//! - `mambalibs.pg.migrate`  → MigrationRunner + Migration (`lib_migrate.rs`)
//!
//! ```python
//! from mambalibs.pg import connect, transaction_begin, transaction_commit
//! from mambalibs.pg.migrate import MigrationRunner, Migration
//! ```

pub mod lib_migrate;
pub mod methods;
pub mod session;
pub mod types;

use mamba_registry::{rt_sym, MambaModule, ModuleRegistrar, MAMBA_MODULES};
use linkme::distributed_slice;

// ── PgMambaModule — `mambalibs.pg` ───────────────────────────────────────────

/// Driver + Transaction surface mounted under `mambalibs.pg`.
pub struct PgMambaModule;

impl MambaModule for PgMambaModule {
    fn name(&self) -> &'static str {
        "mambalibs.pg"
    }

    fn doc(&self) -> &'static str {
        "Mamba interface for pg — driver (Connection) + Transaction"
    }

    fn register(&self, r: &mut ModuleRegistrar) {
        use crate::methods::{
            mb_pg_close, mb_pg_connect, mb_pg_execute, mb_pg_execute_params, mb_pg_ping,
            mb_pg_query_all, mb_pg_query_one, mb_pg_transaction_begin, mb_pg_transaction_commit,
            mb_pg_transaction_execute, mb_pg_transaction_release_savepoint,
            mb_pg_transaction_rollback, mb_pg_transaction_rollback_to, mb_pg_transaction_savepoint,
        };
        use crate::session::{
            mb_pg_session_add, mb_pg_session_begin, mb_pg_session_close, mb_pg_session_commit,
            mb_pg_session_delete, mb_pg_session_flush, mb_pg_session_get, mb_pg_session_new,
            mb_pg_session_query_all, mb_pg_session_query_first, mb_pg_session_rollback,
            mb_pg_session_slot_read, mb_pg_session_touch,
        };

        r.add_symbols([
            // Driver
            rt_sym!("connect", mb_pg_connect, "connect(url: str) -> Connection"),
            rt_sym!("execute", mb_pg_execute, "execute(conn, sql: str) -> int"),
            rt_sym!(
                "execute_params",
                mb_pg_execute_params,
                "execute_params(conn, sql: str, params: list) -> int"
            ),
            rt_sym!(
                "query_all",
                mb_pg_query_all,
                "query_all(conn, sql: str, params: list) -> list[dict]"
            ),
            rt_sym!(
                "query_one",
                mb_pg_query_one,
                "query_one(conn, sql: str, params: list) -> dict?"
            ),
            rt_sym!("ping", mb_pg_ping, "ping(conn) -> None"),
            rt_sym!("close", mb_pg_close, "close(conn) -> None"),
            // Transaction
            rt_sym!(
                "transaction_begin",
                mb_pg_transaction_begin,
                "transaction_begin(conn, isolation='read_committed') -> Transaction"
            ),
            rt_sym!(
                "transaction_execute",
                mb_pg_transaction_execute,
                "transaction_execute(tx, sql: str) -> int"
            ),
            rt_sym!(
                "transaction_savepoint",
                mb_pg_transaction_savepoint,
                "transaction_savepoint(tx, name: str) -> None"
            ),
            rt_sym!(
                "transaction_rollback_to",
                mb_pg_transaction_rollback_to,
                "transaction_rollback_to(tx, name: str) -> None"
            ),
            rt_sym!(
                "transaction_release_savepoint",
                mb_pg_transaction_release_savepoint,
                "transaction_release_savepoint(tx, name: str) -> None"
            ),
            rt_sym!(
                "transaction_commit",
                mb_pg_transaction_commit,
                "transaction_commit(tx) -> None"
            ),
            rt_sym!(
                "transaction_rollback",
                mb_pg_transaction_rollback,
                "transaction_rollback(tx) -> None"
            ),
            // Session / unit-of-work / identity-map
            rt_sym!("Session", mb_pg_session_new, "Session(conn) -> Session"),
            rt_sym!(
                "session_begin",
                mb_pg_session_begin,
                "session_begin(session) -> None"
            ),
            rt_sym!(
                "session_commit",
                mb_pg_session_commit,
                "session_commit(session) -> None"
            ),
            rt_sym!(
                "session_rollback",
                mb_pg_session_rollback,
                "session_rollback(session) -> None"
            ),
            rt_sym!(
                "session_flush",
                mb_pg_session_flush,
                "session_flush(session) -> None"
            ),
            rt_sym!(
                "session_add",
                mb_pg_session_add,
                "session_add(session, table, dict) -> slot"
            ),
            rt_sym!(
                "session_slot_read",
                mb_pg_session_slot_read,
                "session_slot_read(slot) -> int"
            ),
            rt_sym!(
                "session_delete",
                mb_pg_session_delete,
                "session_delete(session, table, pk: int) -> None"
            ),
            rt_sym!(
                "session_touch",
                mb_pg_session_touch,
                "session_touch(session, table, dict_with_id) -> None"
            ),
            rt_sym!(
                "session_get",
                mb_pg_session_get,
                "session_get(session, table, pk: int) -> dict?"
            ),
            rt_sym!(
                "session_query_all",
                mb_pg_session_query_all,
                "session_query_all(session, table, filter) -> list[dict]"
            ),
            rt_sym!(
                "session_query_first",
                mb_pg_session_query_first,
                "session_query_first(session, table, filter) -> dict?"
            ),
            rt_sym!(
                "session_close",
                mb_pg_session_close,
                "session_close(session) -> None"
            ),
        ]);
    }
}

#[distributed_slice(MAMBA_MODULES)]
static PG_MAMBA_MODULE: &dyn MambaModule = &PgMambaModule;
