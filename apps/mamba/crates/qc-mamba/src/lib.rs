//! Mamba binding for `cclab-qc`.
//!
//! Exposes pytest-compatible test decorators to Mamba scripts via the
//! `mamba-registry` infrastructure.
//!
//! # Module name
//!
//! Import in Mamba as `cclab.qc`:
//! ```python
//! from cclab.qc import fixture, mark, raises, parametrize
//! ```
//!
//! # Exposed API
//!
//! | Symbol               | Mamba usage                                     |
//! |----------------------|-------------------------------------------------|
//! | `mb_qc_fixture`      | `@fixture` / `@fixture(autouse=True)` decorator |
//! | `mb_qc_mark`         | `mark` namespace (`.asyncio`, `.parametrize`)   |
//! | `mb_qc_raises`       | `raises(ExcType)` context manager               |
//! | `mb_qc_parametrize`  | `@mark.parametrize(names, cases)` decorator     |

pub mod methods;

use linkme::distributed_slice;
use mamba_registry::{rt_sym, MambaModule, ModuleRegistrar, MAMBA_MODULES};

// ── QcMambaModule ─────────────────────────────────────────────────────────────

/// The `qc-mamba` native module descriptor.
pub struct QcMambaModule;

impl MambaModule for QcMambaModule {
    fn name(&self) -> &'static str {
        "cclab.qc"
    }

    fn doc(&self) -> &'static str {
        "Mamba bindings for cclab-qc — pytest-compatible test fixtures and marks"
    }

    fn register(&self, r: &mut ModuleRegistrar) {
        use crate::methods::{mb_qc_fixture, mb_qc_mark, mb_qc_parametrize, mb_qc_raises};

        r.add_symbols([
            rt_sym!(
                "fixture",
                mb_qc_fixture,
                "fixture(fn, *, autouse: bool = False, scope: str = 'function') -> fn"
            ),
            rt_sym!("mark", mb_qc_mark, "mark() -> mark_namespace"),
            rt_sym!(
                "raises",
                mb_qc_raises,
                "raises(exc_type) -> context_manager"
            ),
            rt_sym!(
                "parametrize",
                mb_qc_parametrize,
                "parametrize(argnames: str, argvalues: list) -> decorator"
            ),
        ]);
    }
}

// ── Auto-registration ─────────────────────────────────────────────────────────

#[distributed_slice(MAMBA_MODULES)]
static QC_MAMBA_MODULE: &dyn MambaModule = &QcMambaModule;
