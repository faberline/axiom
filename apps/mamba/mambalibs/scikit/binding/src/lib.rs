//! Mamba interface for `scikit`.

use linkme::distributed_slice;
use mamba_registry::{MambaModule, ModuleRegistrar, MAMBA_MODULES};

pub struct ScikitMambaModule;

impl MambaModule for ScikitMambaModule {
    fn name(&self) -> &'static str {
        "mambalibs.sci"
    }

    fn doc(&self) -> &'static str {
        "Scikit Mamba-native interface"
    }

    fn register(&self, _r: &mut ModuleRegistrar) {}
}

#[distributed_slice(MAMBA_MODULES)]
static SCIKIT_MAMBA_MODULE: &dyn MambaModule = &ScikitMambaModule;
