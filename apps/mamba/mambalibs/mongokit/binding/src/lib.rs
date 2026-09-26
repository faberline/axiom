//! Mamba interface for `mongokit`.

use mamba_registry::{MambaModule, ModuleRegistrar, MAMBA_MODULES};
use linkme::distributed_slice;

pub struct MongokitMambaModule;

impl MambaModule for MongokitMambaModule {
    fn name(&self) -> &'static str {
        "mambalibs.mongo"
    }

    fn doc(&self) -> &'static str {
        "Mongokit Mamba-native interface"
    }

    fn register(&self, _r: &mut ModuleRegistrar) {}
}

#[distributed_slice(MAMBA_MODULES)]
static MONGOKIT_MAMBA_MODULE: &dyn MambaModule = &MongokitMambaModule;
