//! Blocking (synchronous) façade over the async `pg` API.
//!
//! Each `Blocking*` type wraps the corresponding async type plus an
//! `Arc<tokio::runtime::Runtime>` that drives the async work via
//! `block_on`. Pattern follows `reqwest::blocking`.
//!
//! ## Usage
//!
//! ```rust,ignore
//! use pg::blocking::{Connection, MigrationRunner};
//! use pg::PoolConfig;
//!
//! let conn = Connection::new("postgresql://localhost/db", PoolConfig::default())?;
//! conn.ping()?;
//!
//! let runner = MigrationRunner::new(conn.clone(), None);
//! runner.init()?;
//! ```
//!
//! ## Runtime model
//!
//! Each `BlockingConnection::new` builds a multi-thread tokio runtime
//! wrapped in `Arc<Runtime>`. The runtime is propagated to derived
//! `Blocking{Transaction,MigrationRunner}` via `clone()` so they share
//! the same underlying executor.
//!
//! ## Caveat: nested-block_on panic
//!
//! Calling any `Blocking*` method from inside an existing tokio context
//! (e.g. `#[tokio::main]`) will panic because tokio refuses nested
//! `block_on`. The blocking API is intended for callers that have no
//! tokio runtime of their own — Mamba scripts, plain `fn main()`
//! binaries, FFI consumers. If you are already inside `async`, use the
//! async API at the crate root instead.

mod bulk;
mod connection;
mod executor;
mod migration;
mod row;
mod transaction;

pub use bulk::BulkExecutor;
pub use connection::Connection;
pub use executor::QueryExecutor;
pub use migration::MigrationRunner;
pub use transaction::Transaction;
