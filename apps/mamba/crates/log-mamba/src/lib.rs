//! Mamba binding for `mamba-log`.
//!
//! Exposes structured logging capabilities to Mamba scripts via the
//! `mamba-registry` infrastructure.
//!
//! # Module name
//!
//! Import in Mamba as `cclab.log`:
//! ```python
//! from cclab.log import get_logger
//! ```

pub mod methods;
pub mod types;

use linkme::distributed_slice;
use mamba_registry::{rt_sym, MambaModule, ModuleRegistrar, MAMBA_MODULES};

// ── LogMambaModule ────────────────────────────────────────────────────────────

/// The `log-mamba` native module descriptor.
pub struct LogMambaModule;

impl MambaModule for LogMambaModule {
    fn name(&self) -> &'static str {
        "cclab.log"
    }

    fn doc(&self) -> &'static str {
        "Mamba bindings for mamba-log — structured logging"
    }

    fn register(&self, r: &mut ModuleRegistrar) {
        use crate::methods::{
            mb_log_debug, mb_log_error, mb_log_get_logger, mb_log_info, mb_log_warning,
        };

        r.add_symbols([
            rt_sym!(
                "get_logger",
                mb_log_get_logger,
                "get_logger(name: str) -> logger"
            ),
            rt_sym!("info", mb_log_info, "info(logger, msg: str) -> None"),
            rt_sym!("error", mb_log_error, "error(logger, msg: str) -> None"),
            rt_sym!("debug", mb_log_debug, "debug(logger, msg: str) -> None"),
            rt_sym!(
                "warning",
                mb_log_warning,
                "warning(logger, msg: str) -> None"
            ),
        ]);
    }
}

// ── Auto-registration ─────────────────────────────────────────────────────────

#[distributed_slice(MAMBA_MODULES)]
static LOG_MAMBA_MODULE: &dyn MambaModule = &LogMambaModule;
