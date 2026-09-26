//! Process-wide tokio runtime shared by the mamba interpreter and all native
//! modules that need to dispatch async work (HTTP, DB, MCP, etc.).
//!
//! Hosted here so that every native module crate transitively reaches it
//! through `mamba-registry` (which they already depend on for the
//! `MambaModule` trait), and the `mamba` interpreter itself also reaches it
//! through the same path — avoiding the cycle that would result if the
//! runtime lived in either of those crates.
//!
//! # Usage
//!
//! ```ignore
//! let handle = mamba_registry::runtime::handle();
//! handle.spawn(async { /* ... */ });
//! ```

use once_cell::sync::Lazy;
use tokio::runtime::{Builder, Handle, Runtime};

static RUNTIME: Lazy<Runtime> = Lazy::new(|| {
    Builder::new_multi_thread()
        .enable_all()
        .thread_name("mamba-rt")
        .build()
        .expect("failed to build mamba shared tokio runtime")
});

/// Handle to the process-wide mamba tokio runtime.
///
/// First call materializes the runtime; subsequent calls return the same
/// handle. Safe to call from any thread, sync or async context.
pub fn handle() -> Handle {
    RUNTIME.handle().clone()
}

/// Block on a future using the shared runtime.
///
/// Convenience for sync entry points (e.g. Mamba FFI shims) that need to drive
/// an async call to completion. Do not call from inside an async context — use
/// [`handle`] and `spawn`/`await` instead.
pub fn block_on<F: std::future::Future>(future: F) -> F::Output {
    RUNTIME.block_on(future)
}
