#![cfg(test)]

// Schema gate for #2095 — qc-mamba binding. Locks the
// manifest shape only; does NOT build mamba runtime.

use std::fs;
use std::path::PathBuf;
use toml::Value;

fn manifest_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/governance/gates/infra/cclab_qc_mamba_binding_gate/manifest.toml")
}

fn manifest() -> Value {
    let raw = fs::read_to_string(manifest_path()).expect("read manifest");
    raw.parse::<Value>().expect("parse manifest toml")
}

fn get<'a>(v: &'a Value, key: &str) -> &'a Value {
    v.get(key).unwrap_or_else(|| panic!("missing key: {key}"))
}
fn b(v: &Value, key: &str) -> bool {
    get(v, key)
        .as_bool()
        .unwrap_or_else(|| panic!("{key} not bool"))
}
fn s<'a>(v: &'a Value, key: &str) -> &'a str {
    get(v, key)
        .as_str()
        .unwrap_or_else(|| panic!("{key} not str"))
}
fn i(v: &Value, key: &str) -> i64 {
    get(v, key)
        .as_integer()
        .unwrap_or_else(|| panic!("{key} not int"))
}
fn a<'a>(v: &'a Value, key: &str) -> &'a Vec<Value> {
    get(v, key)
        .as_array()
        .unwrap_or_else(|| panic!("{key} not array"))
}

#[test]
fn header_is_well_formed() {
    let m = manifest();
    assert_eq!(i(&m, "version"), 1);
    assert_eq!(s(&m, "fixture"), "cclab_qc_mamba_binding_gate");
    assert_eq!(i(&m, "issue"), 2095);
    assert_eq!(i(&m, "parent_issue"), 2093);
    assert_eq!(s(&m, "profile"), "conformance");
    assert_eq!(s(&m, "family"), "cclab_qc_mamba_binding_gate");
    assert_eq!(s(&m, "network"), "offline");
}

#[test]
fn isolation_pins_no_global_state() {
    let m = manifest();
    let iso = get(&m, "isolation");
    assert!(b(iso, "forbid_writes_outside_project"));
    assert!(b(iso, "forbid_user_home_reads"));
    assert!(b(iso, "forbid_global_cache_reads"));
    assert!(b(iso, "forbid_global_cache_writes"));
}

#[test]
fn python_target_is_pinned_to_3_12() {
    let m = manifest();
    let py = get(&m, "python_target");
    assert_eq!(i(py, "python_major"), 3);
    assert_eq!(i(py, "python_minor"), 12);
    assert!(b(py, "must_be_python_3_12"));
}

#[test]
fn surface_pins_the_five_required_axes() {
    let m = manifest();
    let sf = get(&m, "surface");
    assert!(b(sf, "must_cover_pyo3_module_exposes_cprofile_shaped_api"));
    assert!(b(sf, "must_cover_pyspy_sampling_bridge_walks_jit_frames"));
    assert!(b(sf, "must_cover_phase_breakdown_attribution_six_buckets"));
    assert!(b(sf, "must_cover_zero_overhead_when_disabled"));
    assert!(b(sf, "must_cover_correctness_invariants_recorded"));
    assert!(b(sf, "must_be_offline_or_loopback_only"));
    assert!(b(sf, "must_be_deterministic"));
}

#[test]
fn r1_pyo3_cprofile_shaped_api() {
    let m = manifest();
    let r = get(&m, "r1_pyo3_cprofile_shaped_api_contract");
    assert_eq!(s(r, "requirement_id"), "R1");
    assert!(b(r, "must_expose_enable_function"));
    assert!(b(r, "must_expose_disable_function"));
    assert!(b(r, "must_expose_get_stats_function"));
    assert!(b(r, "must_expose_context_manager_convenience_wrapper"));
    assert!(b(r, "must_be_importable_under_mamba"));
    assert!(b(
        r,
        "forbid_requiring_separate_process_for_basic_profiling"
    ));
    assert_eq!(s(r, "expected_pyo3_module_name"), "cclab.qc");
    let api = a(r, "required_api_surface");
    let names: Vec<&str> = api.iter().filter_map(|v| v.as_str()).collect();
    for f in &["enable", "disable", "get_stats", "profile_context_manager"] {
        assert!(names.contains(f), "missing api: {f}");
    }
    assert_eq!(i(r, "import_fails_under_mamba_exit_code"), 488);
    assert_eq!(i(r, "api_surface_missing_exit_code"), 489);
    assert!(b(r, "must_distinguish_import_fail_from_api_missing"));
}

#[test]
fn r2_pyspy_sampling_bridge_walks_jit_frames() {
    let m = manifest();
    let r = get(&m, "r2_pyspy_sampling_bridge_walks_jit_frames_contract");
    assert_eq!(s(r, "requirement_id"), "R2");
    assert!(b(r, "must_walk_mamba_jit_frames"));
    assert!(b(r, "must_emit_jit_frame_with_python_source_location"));
    assert!(b(r, "forbid_emitting_unknown_jit_frame_placeholder"));
    assert!(b(r, "forbid_dropping_jit_frame_from_stack_sample"));
    let modes = a(r, "allowed_sampling_mode_values");
    let mnames: Vec<&str> = modes.iter().filter_map(|v| v.as_str()).collect();
    assert!(mnames.contains(&"py_spy_compatible_sampling"));
    assert!(mnames.contains(&"cprofile_deterministic"));
    assert_eq!(
        s(r, "expected_sampling_mode_for_r2"),
        "py_spy_compatible_sampling"
    );
    assert_eq!(i(r, "unknown_jit_frame_exit_code"), 490);
    assert_eq!(i(r, "jit_frame_dropped_exit_code"), 491);
    assert!(b(r, "must_distinguish_unknown_frame_from_dropped_frame"));
}

#[test]
fn r3_phase_breakdown_attribution_six_buckets() {
    let m = manifest();
    let r = get(&m, "r3_phase_breakdown_attribution_six_buckets_contract");
    assert_eq!(s(r, "requirement_id"), "R3");
    assert!(b(r, "must_define_six_canonical_buckets"));
    assert!(b(r, "must_attribute_every_sample_to_exactly_one_bucket"));
    assert!(b(r, "forbid_double_counting_across_buckets"));
    assert!(b(r, "forbid_leaving_sample_unattributed_silently"));
    let buckets = a(r, "canonical_buckets");
    let names: Vec<&str> = buckets.iter().filter_map(|v| v.as_str()).collect();
    for k in &[
        "PythonExtract",
        "RustConvert",
        "JITCompile",
        "JITExec",
        "GC",
        "Other",
    ] {
        assert!(names.contains(k), "missing bucket: {k}");
    }
    assert_eq!(buckets.len(), 6);
    assert_eq!(i(r, "double_count_exit_code"), 492);
    assert_eq!(i(r, "sample_unattributed_exit_code"), 493);
    assert_eq!(i(r, "bucket_missing_exit_code"), 494);
    assert!(b(
        r,
        "must_distinguish_double_count_from_unattributed_from_bucket_missing"
    ));
}

#[test]
fn r4_zero_overhead_when_disabled() {
    let m = manifest();
    let r = get(&m, "r4_zero_overhead_when_disabled_contract");
    assert_eq!(s(r, "requirement_id"), "R4");
    assert!(b(r, "must_compile_out_when_feature_off"));
    assert!(b(r, "must_keep_idle_session_overhead_under_5_ns_per_call"));
    assert!(b(r, "forbid_unconditional_hot_path_call_to_session_check"));
    assert!(b(r, "forbid_silently_widening_overhead_budget"));
    let kinds = a(r, "allowed_feature_off_compile_kind_values");
    let knames: Vec<&str> = kinds.iter().filter_map(|v| v.as_str()).collect();
    assert!(knames.contains(&"no_op_when_feature_off"));
    assert!(knames.contains(&"always_compiled_in"));
    assert_eq!(
        s(r, "expected_feature_off_compile_kind"),
        "no_op_when_feature_off"
    );
    assert_eq!(i(r, "expected_idle_session_overhead_budget_ns"), 5);
    assert_eq!(i(r, "feature_off_not_compiled_out_exit_code"), 495);
    assert_eq!(i(r, "idle_session_overhead_exceeded_exit_code"), 496);
    assert_eq!(i(r, "overhead_budget_widened_exit_code"), 497);
    assert!(b(
        r,
        "must_distinguish_not_compiled_out_from_exceeded_from_widened"
    ));
}

#[test]
fn r5_correctness_invariants_recorded() {
    let m = manifest();
    let r = get(&m, "r5_correctness_invariants_recorded_contract");
    assert_eq!(s(r, "requirement_id"), "R5");
    assert!(b(r, "must_record_sample_count"));
    assert!(b(r, "must_record_warmup_iters"));
    assert!(b(r, "must_record_elapsed_ns"));
    assert!(b(
        r,
        "must_assert_two_consecutive_sessions_within_10_percent"
    ));
    assert!(b(r, "forbid_dropping_invariant_field_from_session"));
    let fields = a(r, "required_session_fields");
    let names: Vec<&str> = fields.iter().filter_map(|v| v.as_str()).collect();
    for f in &["sample_count", "warmup_iters", "elapsed_ns"] {
        assert!(names.contains(f), "missing field: {f}");
    }
    assert_eq!(
        s(r, "expected_consecutive_session_noise_tolerance"),
        "10_percent"
    );
    assert_eq!(i(r, "invariant_field_missing_exit_code"), 498);
    assert_eq!(i(r, "consecutive_session_diverged_exit_code"), 499);
    assert!(b(r, "must_distinguish_field_missing_from_session_diverged"));
}

#[test]
fn runner_contract_declares_keys_and_cases() {
    let m = manifest();
    let rc = get(&m, "runner_contract");
    let keys = a(rc, "keys");
    let names: Vec<&str> = keys.iter().filter_map(|v| v.as_str()).collect();
    for k in &[
        "outcome",
        "case",
        "requirement_id",
        "pyo3_module_name",
        "required_api_surface",
        "sampling_mode",
        "canonical_buckets",
        "feature_off_compile_kind",
        "idle_session_overhead_budget_ns",
        "required_session_fields",
        "consecutive_session_noise_tolerance",
        "failure_kind",
        "exit_code",
    ] {
        assert!(names.contains(k), "missing key: {k}");
    }
    let cases = a(rc, "case_values");
    assert_eq!(cases.len(), 5);
}

#[test]
fn pins_out_of_scope_per_issue() {
    let m = manifest();
    let oos = get(&m, "out_of_scope");
    assert!(b(oos, "implementation_of_cclab_qc_profiler_itself"));
    assert!(b(oos, "rewriting_cprofile_or_pyspy_upstream"));
    assert!(b(oos, "saas_upload_or_cross_machine_aggregation"));
    assert!(b(oos, "non_mamba_runtime_profiler_targets"));
    assert!(b(oos, "c_extension_fast_paths"));
}
