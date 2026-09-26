//! wgpu runtime-validation gate + `log` → `tracing` bridge.
//!
//! Why this module exists: wgpu ships two complementary safety nets.
//! Rust's type system covers the compile-time half — wrong descriptor
//! variants, lifetime escapes — but the *runtime* validation layer is
//! the only thing that catches API-misuse errors only the driver can
//! see (unbound buffer index, render-pass missing a pipeline, texture
//! used in a state incompatible with its descriptor). The validation
//! layer is far too slow to ship in release builds (it shadows every
//! call with full state tracking), but it is exactly what we want
//! during development and in CI.
//!
//! Invariant — explicit `cfg(debug_assertions)` gate: wgpu's own
//! `InstanceFlags::default()` calls `from_build_config()` which already
//! gates on `cfg!(debug_assertions)`, but that contract is *implicit*
//! — a future refactor that drops `..Default::default()` on the
//! `InstanceDescriptor` would silently disable validation across the
//! entire renderer with no compile-time signal. This module's
//! [`instance_flags_for_build`] reads `cfg!(debug_assertions)`
//! literally and the renderer's `new()` consumes the result by name;
//! both are grep-visible and the gate is impossible to lose silently.
//!
//! Invariant — opt-in log → tracing bridge: wgpu's validation errors
//! reach userland through the `log` crate. Downstream consumers (and
//! our test harness) increasingly standardize on `tracing`, so
//! validation diagnostics would vanish into the void without a
//! shim. [`try_install_log_bridge`] is a thin wrapper around
//! `tracing_log::LogTracer::init()` — idempotent in spirit; if a
//! `log` global is already installed the underlying error is returned
//! verbatim so the caller decides whether that's fatal.

/// Return the `wgpu::InstanceFlags` appropriate for the current build:
/// `DEBUG | VALIDATION` under `cfg(debug_assertions)`, `empty()`
/// otherwise.
///
/// Pure — testable without a GPU. The whole point of this helper is
/// that the renderer's `InstanceDescriptor` reads the gate by name
/// instead of relying on `..Default::default()`'s implicit
/// `from_build_config()` call.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/wgpu-validation-dev-slice-4k.md#interface
/// @issue #1729
pub fn instance_flags_for_build() -> wgpu::InstanceFlags {
    if cfg!(debug_assertions) {
        wgpu::InstanceFlags::debugging()
    } else {
        wgpu::InstanceFlags::empty()
    }
}

/// Install `tracing-log::LogTracer` as the global `log` shim so
/// `log::error!()` calls inside wgpu (validation diagnostics in
/// particular) reach the active `tracing::Subscriber`.
///
/// Returns the underlying error if a `log` global was already
/// installed — many test harnesses install a `log` subscriber for
/// other crates and that is not necessarily fatal. The helper is
/// idempotent in spirit: subsequent calls are no-ops via the error
/// arm and the caller decides whether to treat the duplicate
/// installation as a hard failure.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/wgpu-validation-dev-slice-4k.md#interface
/// @issue #1729
pub fn try_install_log_bridge() -> Result<(), tracing_log::log_tracer::SetLoggerError> {
    tracing_log::LogTracer::init()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_match_build_config() {
        // Under `cargo test` (which enables `debug_assertions` by
        // default) the helper must return DEBUG | VALIDATION. Under a
        // hypothetical `--release` test it must return empty. Branch
        // on `cfg!(debug_assertions)` so the assertion is exhaustive
        // in either build configuration.
        let flags = instance_flags_for_build();
        if cfg!(debug_assertions) {
            assert!(
                flags.contains(wgpu::InstanceFlags::VALIDATION),
                "debug build must enable VALIDATION; got {flags:?}"
            );
            assert!(
                flags.contains(wgpu::InstanceFlags::DEBUG),
                "debug build must enable DEBUG; got {flags:?}"
            );
        } else {
            assert!(
                flags.is_empty(),
                "release build must produce empty flags; got {flags:?}"
            );
        }
    }

    #[test]
    fn flags_equal_wgpu_debugging_alias_under_debug() {
        // `InstanceFlags::debugging()` is wgpu's documented alias for
        // `DEBUG | VALIDATION`. Lock the equivalence in so a future
        // wgpu API shake-up that splits the two surfaces a test
        // failure rather than a silently weaker dev gate.
        if cfg!(debug_assertions) {
            assert_eq!(instance_flags_for_build(), wgpu::InstanceFlags::debugging());
        }
    }

    #[test]
    fn log_bridge_install_is_idempotent_in_spirit() {
        // First call may succeed or fail depending on whether another
        // test in this binary has already installed a `log` global —
        // tests run in arbitrary order. Whichever outcome we see, the
        // SECOND call must error (the `log` global is now installed).
        let _first = try_install_log_bridge();
        let second = try_install_log_bridge();
        assert!(
            second.is_err(),
            "second install_log_bridge call must error (log global already set)"
        );
    }
}
