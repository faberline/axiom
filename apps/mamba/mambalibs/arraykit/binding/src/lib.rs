//! Mamba interface for `arraykit`.
//!
//! Core source and logic live in the sibling `arraykit` crate. This crate owns
//! the Mamba import namespace `mambalibs.array` and the native binding surface.
//!
//! The symbol/value registration surface is intentionally empty at scaffold
//! time. Subsequent work (the dedup-against-mamba pass tracked in the
//! migration plan) will populate this with `array_mod`-equivalent symbols
//! that delegate to `arraykit::array` types and reuse mamba's `MbValue` /
//! `RuntimeValue` definitions instead of redefining them here.

use linkme::distributed_slice;
use mamba_registry::{MambaModule, ModuleRegistrar, MAMBA_MODULES};

pub struct ArraykitMambaModule;

impl MambaModule for ArraykitMambaModule {
    fn name(&self) -> &'static str {
        "mambalibs.array"
    }

    fn doc(&self) -> &'static str {
        "Arraykit Mamba-native interface"
    }

    fn register(&self, _r: &mut ModuleRegistrar) {
        // Symbol/value registration deferred to the dedup pass.
    }
}

#[distributed_slice(MAMBA_MODULES)]
static ARRAYKIT_MAMBA_MODULE: &dyn MambaModule = &ArraykitMambaModule;
