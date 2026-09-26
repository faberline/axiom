//! Mamba interface for `cryptokit`.
//!
//! Core source and logic live in the sibling `cryptokit` crate. This crate
//! owns the Mamba import namespace `mambalibs.crypto` and the native binding
//! surface. Symbol/value registration is deferred to the dedup pass.

use linkme::distributed_slice;
use mamba_registry::{MambaModule, ModuleRegistrar, MAMBA_MODULES};

pub struct CryptokitMambaModule;

impl MambaModule for CryptokitMambaModule {
    fn name(&self) -> &'static str {
        "mambalibs.crypto"
    }

    fn doc(&self) -> &'static str {
        "Cryptokit Mamba-native interface"
    }

    fn register(&self, _r: &mut ModuleRegistrar) {}
}

#[distributed_slice(MAMBA_MODULES)]
static CRYPTOKIT_MAMBA_MODULE: &dyn MambaModule = &CryptokitMambaModule;
