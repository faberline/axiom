// Force-link Mamba native binding crates so their #[distributed_slice(MAMBA_MODULES)]
// entries are pulled into the binary. Without these `use … as _;` lines the
// linker drops the whole crate; the linkme slice ends up missing the entry and
// `mamba run` cannot resolve imports like `from mambalibs.pg import connect`.
//
// Every kit added here must also appear in `EXPECTED_KITS` so the startup
// self-check (`assert_all_registered`) catches a silently-missing registration
// before user code runs.

#[cfg(feature = "native-modules")]
use agentkit_binding as _;
#[cfg(feature = "native-modules")]
use log_mamba as _;
#[cfg(feature = "native-modules")]
use mcp_mamba as _;
#[cfg(feature = "native-modules")]
use qc_mamba as _;
#[cfg(feature = "native-modules")]
use schema_mamba as _;
#[cfg(feature = "native-modules")]
use mambalibs_di_binding as _;
#[cfg(feature = "native-modules")]
use mambalibs_http_binding as _;
#[cfg(feature = "native-modules")]
use pgkit_binding as _;

/// Canonical Python-level module names every force-linked kit registers via
/// `MambaModule::name()`. Kept in lockstep with the `use … as _;` block above.
#[cfg(feature = "native-modules")]
pub const EXPECTED_KITS: &[&str] = &[
    "cclab.agent",
    "mambalibs.dataclasses",
    "mambalibs.di",
    "mambalibs.http",
    "cclab.log",
    "cclab.mcp",
    "mambalibs.pg",
    "cclab.qc",
];

#[cfg(not(feature = "native-modules"))]
pub const EXPECTED_KITS: &[&str] = &[];

/// Startup self-check: every name in `EXPECTED_KITS` must appear in the
/// `MAMBA_MODULES` distributed slice. Linkme failures are silent — a missing
/// `use … as _;` simply drops the kit — so we trip loudly here instead of
/// leaving the user with a confusing `ModuleNotFoundError` deep inside a
/// `from <kit> import …`.
pub fn assert_all_registered() {
    use std::collections::HashSet;
    let registered: HashSet<&'static str> = mamba_registry::MAMBA_MODULES
        .iter()
        .map(|m| m.name())
        .collect();
    let mut missing: Vec<&'static str> = EXPECTED_KITS
        .iter()
        .copied()
        .filter(|n| !registered.contains(n))
        .collect();
    if !missing.is_empty() {
        missing.sort();
        panic!(
            "mamba startup: expected MambaModule kits not registered: {:?} \
             (force-link table out of sync with EXPECTED_KITS in \
             pkgmanage::builder::force_link)",
            missing,
        );
    }
}
