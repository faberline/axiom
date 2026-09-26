//! Mamba interface for `plotkit`.

use linkme::distributed_slice;
use mamba_registry::{MambaModule, ModuleRegistrar, MAMBA_MODULES};

pub struct PlotkitMambaModule;

impl MambaModule for PlotkitMambaModule {
    fn name(&self) -> &'static str {
        "mambalibs.plot"
    }

    fn doc(&self) -> &'static str {
        "Plotkit Mamba-native interface"
    }

    fn register(&self, _r: &mut ModuleRegistrar) {}
}

#[distributed_slice(MAMBA_MODULES)]
static PLOTKIT_MAMBA_MODULE: &dyn MambaModule = &PlotkitMambaModule;
