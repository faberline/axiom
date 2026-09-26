//! Schema gate for the mambalibs local Mode 2 binding crate fixture —
//! closes #2577.
//!
//! Acceptance (issue #2577):
//!
//!   1. Fixture crate builds independently.
//!      `[independent_build_case]` pins
//!      must_build_with_no_workspace_member_dependency,
//!      must_not_depend_on_cclab_production_crates,
//!      must_compile_offline, and a fast_build_budget_seconds floor.
//!   2. Exported function returns a value that can be asserted from
//!      Python. `[python_assertion_case]` pins the user-facing
//!      import, must_assert_returned_value_exactly = true, and the
//!      asserted_value cross-references [exported_function_contract].
//!   3. No cclab production crate behavior is changed.
//!      `[production_crate_isolation]` pins
//!      must_not_modify_cclab_production_crates AND a forbidden list
//!      that includes the core production crates.

use std::path::{Path, PathBuf};

fn manifest_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("mambalibs")
        .join("fixtures")
        .join("local_mode2_binding_crate")
        .join("manifest.toml")
}

fn load_toml(path: &Path) -> toml::Value {
    let raw = std::fs::read_to_string(path).unwrap();
    raw.parse().unwrap()
}

#[test]
fn header_is_well_formed() {
    let doc = load_toml(&manifest_path());
    assert_eq!(
        doc.get("fixture").and_then(|v| v.as_str()),
        Some("mambalibs_local_mode2_binding_crate"),
    );
    assert_eq!(doc.get("issue").and_then(|v| v.as_integer()), Some(2577));
    assert_eq!(
        doc.get("parent_issue").and_then(|v| v.as_integer()),
        Some(2531)
    );
    assert_eq!(
        doc.get("profile").and_then(|v| v.as_str()),
        Some("mambalibs")
    );
    assert_eq!(
        doc.get("family").and_then(|v| v.as_str()),
        Some("local_mode2_binding_crate")
    );
    assert_eq!(doc.get("network").and_then(|v| v.as_str()), Some("offline"));
}

#[test]
fn isolation_pins_no_global_state() {
    let doc = load_toml(&manifest_path());
    let i = doc.get("isolation").and_then(|v| v.as_table()).unwrap();
    for f in &[
        "forbid_writes_outside_project",
        "forbid_user_home_reads",
        "forbid_global_cache_reads",
        "forbid_global_cache_writes",
    ] {
        assert_eq!(i.get(*f).and_then(|v| v.as_bool()), Some(true));
    }
}

#[test]
fn crate_skeleton_is_small_local_pyo3_cdylib() {
    let doc = load_toml(&manifest_path());
    let c = doc
        .get("crate")
        .and_then(|v| v.as_table())
        .expect("[crate] missing");
    assert!(!c
        .get("crate_name")
        .and_then(|v| v.as_str())
        .unwrap()
        .is_empty());
    assert_eq!(c.get("crate_type").and_then(|v| v.as_str()), Some("cdylib"));
    assert_eq!(c.get("binding_kind").and_then(|v| v.as_str()), Some("pyo3"));
    assert_eq!(
        c.get("module_name").and_then(|v| v.as_str()),
        Some("mambalibs")
    );
    let exported = c.get("exported_function").and_then(|v| v.as_str()).unwrap();
    assert!(!exported.is_empty(), "must declare an exported_function");
    let stmt = c.get("import_statement").and_then(|v| v.as_str()).unwrap();
    assert!(stmt.starts_with("from mambalibs import "));
    assert!(stmt.contains(exported));
    assert_eq!(
        c.get("must_be_small_local_fixture")
            .and_then(|v| v.as_bool()),
        Some(true)
    );
    assert_eq!(
        c.get("documented_as_canonical_mode2_dependency")
            .and_then(|v| v.as_bool()),
        Some(true)
    );
}

#[test]
fn exported_function_contract_pins_one_deterministic_return() {
    let doc = load_toml(&manifest_path());
    let e = doc
        .get("exported_function_contract")
        .and_then(|v| v.as_table())
        .expect("[exported_function_contract] missing");
    let name = e.get("function_name").and_then(|v| v.as_str()).unwrap();
    assert!(!name.is_empty());
    assert_eq!(
        e.get("return_kind").and_then(|v| v.as_str()),
        Some("integer")
    );
    assert!(e.get("return_value").and_then(|v| v.as_integer()).is_some());
    assert_eq!(
        e.get("must_be_deterministic").and_then(|v| v.as_bool()),
        Some(true)
    );
    assert_eq!(
        e.get("must_be_assertable_from_python")
            .and_then(|v| v.as_bool()),
        Some(true)
    );
    assert_eq!(
        e.get("assertion_kind").and_then(|v| v.as_str()),
        Some("exact_equality")
    );

    // Cross-check function_name matches [crate].exported_function.
    let crate_exported = doc
        .get("crate")
        .and_then(|v| v.get("exported_function"))
        .and_then(|v| v.as_str())
        .unwrap();
    assert_eq!(
        name, crate_exported,
        "function_name must match [crate].exported_function"
    );
}

// Acceptance: "Fixture crate builds independently."
#[test]
fn fixture_crate_builds_independently() {
    let doc = load_toml(&manifest_path());
    let c = doc
        .get("independent_build_case")
        .and_then(|v| v.as_table())
        .expect(
            "[independent_build_case] missing — acceptance: \
         \"Fixture crate builds independently.\"",
        );
    assert_eq!(
        c.get("must_build_with_no_workspace_member_dependency")
            .and_then(|v| v.as_bool()),
        Some(true)
    );
    assert_eq!(
        c.get("must_not_depend_on_cclab_production_crates")
            .and_then(|v| v.as_bool()),
        Some(true)
    );
    assert_eq!(
        c.get("must_compile_offline").and_then(|v| v.as_bool()),
        Some(true)
    );
    assert_eq!(
        c.get("expected_outcome").and_then(|v| v.as_str()),
        Some("pass")
    );
    assert_eq!(
        c.get("expected_exit_code").and_then(|v| v.as_integer()),
        Some(0)
    );
    assert_eq!(
        c.get("build_failure_outcome").and_then(|v| v.as_str()),
        Some("fail")
    );
    assert_eq!(
        c.get("build_failure_exit_code")
            .and_then(|v| v.as_integer()),
        Some(1)
    );
    assert_eq!(c.get("must_be_fast").and_then(|v| v.as_bool()), Some(true));
    let budget = c
        .get("fast_build_budget_seconds")
        .and_then(|v| v.as_integer())
        .unwrap();
    assert!(budget > 0, "fast_build_budget_seconds must be positive");
}

// Acceptance: "Exported function returns a value that can be asserted
// from Python."
#[test]
fn exported_function_value_is_assertable_from_python() {
    let doc = load_toml(&manifest_path());
    let c = doc
        .get("python_assertion_case")
        .and_then(|v| v.as_table())
        .expect(
            "[python_assertion_case] missing — acceptance: \
         \"Exported function returns a value that can be asserted from Python.\"",
        );
    let stmt = c
        .get("must_use_user_facing_import")
        .and_then(|v| v.as_str())
        .unwrap();
    assert!(stmt.starts_with("from mambalibs import "));
    let must_call = c
        .get("must_call_function")
        .and_then(|v| v.as_str())
        .unwrap();
    let crate_exported = doc
        .get("crate")
        .and_then(|v| v.get("exported_function"))
        .and_then(|v| v.as_str())
        .unwrap();
    assert_eq!(must_call, crate_exported);
    assert_eq!(
        c.get("must_assert_returned_value_exactly")
            .and_then(|v| v.as_bool()),
        Some(true)
    );
    let asserted = c
        .get("asserted_value")
        .and_then(|v| v.as_integer())
        .unwrap();
    let contract_return = doc
        .get("exported_function_contract")
        .and_then(|v| v.get("return_value"))
        .and_then(|v| v.as_integer())
        .unwrap();
    assert_eq!(
        asserted, contract_return,
        "asserted_value must match [exported_function_contract].return_value"
    );
    assert_eq!(
        c.get("expected_outcome").and_then(|v| v.as_str()),
        Some("pass")
    );
    assert_eq!(
        c.get("expected_exit_code").and_then(|v| v.as_integer()),
        Some(0)
    );
    assert_eq!(
        c.get("mismatch_outcome").and_then(|v| v.as_str()),
        Some("fail")
    );
    assert_eq!(
        c.get("mismatch_exit_code").and_then(|v| v.as_integer()),
        Some(3)
    );
}

// Acceptance: "No cclab production crate behavior is changed."
#[test]
fn no_cclab_production_crate_behavior_is_changed() {
    let doc = load_toml(&manifest_path());
    let p = doc
        .get("production_crate_isolation")
        .and_then(|v| v.as_table())
        .expect(
            "[production_crate_isolation] missing — acceptance: \
         \"No cclab production crate behavior is changed.\"",
        );
    assert_eq!(
        p.get("must_not_modify_cclab_production_crates")
            .and_then(|v| v.as_bool()),
        Some(true)
    );
    assert_eq!(
        p.get("must_be_test_only").and_then(|v| v.as_bool()),
        Some(true)
    );
    assert_eq!(
        p.get("must_live_under_tests_fixtures_dir")
            .and_then(|v| v.as_bool()),
        Some(true)
    );
    let forbidden: Vec<&str> = p
        .get("forbidden_modifications")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();
    for required in &["cclab-mamba", "cclab-runtime", "cclab-jet", "mamba-core"] {
        assert!(
            forbidden.contains(required),
            "forbidden_modifications must include {required}"
        );
    }
}

#[test]
fn runner_contract_declares_keys_and_cases() {
    let doc = load_toml(&manifest_path());
    let c = doc
        .get("runner_contract")
        .and_then(|v| v.as_table())
        .unwrap();
    let keys: Vec<&str> = c
        .get("keys")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();
    for required in &[
        "outcome",
        "case",
        "crate_name",
        "exported_function",
        "returned_value",
        "expected_returned_value",
        "build_seconds",
        "exit_code",
    ] {
        assert!(
            keys.contains(required),
            "runner_contract.keys must include {required}"
        );
    }
    let cases: Vec<&str> = c
        .get("case_values")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();
    for required in &[
        "binding_crate_builds_independently",
        "exported_function_assertable_from_python",
    ] {
        assert!(
            cases.contains(required),
            "runner_contract.case_values must include {required}"
        );
    }
}

#[test]
fn pins_out_of_scope_per_issue() {
    let doc = load_toml(&manifest_path());
    let o = doc.get("out_of_scope").and_then(|v| v.as_table()).unwrap();
    assert_eq!(
        o.get("testing_every_cclab_library")
            .and_then(|v| v.as_bool()),
        Some(true)
    );
}
