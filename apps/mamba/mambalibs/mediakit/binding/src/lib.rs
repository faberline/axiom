//! Mamba interface for `mediakit`.

use mamba_registry::{MambaModule, ModuleRegistrar, MAMBA_MODULES};
use linkme::distributed_slice;

pub struct MediakitMambaModule;

impl MambaModule for MediakitMambaModule {
    fn name(&self) -> &'static str {
        "mambalibs.media"
    }

    fn doc(&self) -> &'static str {
        "Mediakit Mamba-native interface"
    }

    fn register(&self, _r: &mut ModuleRegistrar) {}
}

#[distributed_slice(MAMBA_MODULES)]
static MEDIAKIT_MAMBA_MODULE: &dyn MambaModule = &MediakitMambaModule;
