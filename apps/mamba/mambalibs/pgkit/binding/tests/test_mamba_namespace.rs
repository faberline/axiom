//! Namespace registration tests for the pgkit Mamba interface.

use mamba_registry::{find_module, ModuleRegistrar};
use pgkit_binding as _;
use std::collections::HashSet;

#[test]
fn registers_mambalibs_pg_namespace() {
    let module = find_module("mambalibs.pg").expect("pgkit must register mambalibs.pg");
    let mut registrar = ModuleRegistrar::new();
    module.register(&mut registrar);

    let symbols: HashSet<&str> = registrar.symbols().iter().map(|sym| sym.name).collect();
    for expected in [
        "connect",
        "execute",
        "execute_params",
        "query_all",
        "query_one",
        "transaction_begin",
        "Session",
    ] {
        assert!(
            symbols.contains(expected),
            "mambalibs.pg missing symbol {expected}"
        );
    }

    assert!(
        find_module("cclab.pg").is_none(),
        "cclab.pg is a legacy namespace and must not be registered"
    );
}

#[test]
fn registers_mambalibs_pg_migrate_namespace() {
    let module =
        find_module("mambalibs.pg.migrate").expect("pgkit must register mambalibs.pg.migrate");
    let mut registrar = ModuleRegistrar::new();
    module.register(&mut registrar);

    let symbols: HashSet<&str> = registrar.symbols().iter().map(|sym| sym.name).collect();
    for expected in ["MigrationRunner", "runner_status", "Migration"] {
        assert!(
            symbols.contains(expected),
            "mambalibs.pg.migrate missing symbol {expected}"
        );
    }

    assert!(
        find_module("cclab.pg.migrate").is_none(),
        "cclab.pg.migrate is a legacy namespace and must not be registered"
    );
}
