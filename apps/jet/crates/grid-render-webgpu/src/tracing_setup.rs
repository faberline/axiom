//! Default `tracing` subscriber installation for the WebGPU renderer.
//! Slice 4x (#1742).
//!
//! Why this module exists: the per-frame / per-pass `tracing` spans the
//! renderer now emits (`frame`, `cell_pass`, future `text_pass`) are only
//! useful if *something* is collecting them. The application that hosts
//! the renderer is the natural owner of subscriber wiring — production
//! apps want JSON-to-stdout, devtools wants chrome://tracing JSON, tests
//! want a no-op fmt subscriber. But the JS bridge, headless tests, and
//! CLI binaries that drive this crate all want the same uniform default:
//! a `tracing_subscriber::fmt` subscriber configured from `RUST_LOG` via
//! [`tracing_subscriber::EnvFilter`].
//!
//! Rather than have each caller duplicate that wiring (and risk subtle
//! divergence between which `RUST_LOG` shape they accept), this module
//! exposes a single shared helper: [`install_default_subscriber`]. It is
//! idempotent at *call* level — second and subsequent callers in the
//! same process get [`SubscriberInstallError::AlreadyInstalled`] back,
//! which callers should treat as a no-op.
//!
//! Apps that want a non-default subscriber should *not* call this — they
//! should install their own with the global `tracing::subscriber::set_global_default`
//! before any renderer code runs.

use thiserror::Error;
use tracing_subscriber::EnvFilter;

/// Failure mode of [`install_default_subscriber`]. The only variant is
/// `AlreadyInstalled` — a different global subscriber (or a previous
/// call to this helper) won the install race.
///
/// Callers that drive this from tests should treat `AlreadyInstalled`
/// as a no-op: the spans they emit will still flow to whichever
/// subscriber was installed first.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/tracing-instrumentation-per-pass-spans-slice-4x.md#interface
/// @issue #1742
#[derive(Debug, Error)]
pub enum SubscriberInstallError {
    #[error("a global tracing subscriber is already installed")]
    AlreadyInstalled,
}

/// Install a `tracing_subscriber::fmt` subscriber filtered by
/// [`EnvFilter::from_default_env`] (reads `RUST_LOG`). Returns
/// `Err(AlreadyInstalled)` if a global subscriber is already in place —
/// callers should treat that as a no-op.
///
/// `RUST_LOG` examples:
/// - `RUST_LOG=info` — info+ globally.
/// - `RUST_LOG=jet_grid_render_webgpu=trace` — trace+ for this crate
///   only.
/// - unset / empty — fmt's silent-by-default behavior takes over (no
///   output).
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/tracing-instrumentation-per-pass-spans-slice-4x.md#interface
/// @issue #1742
pub fn install_default_subscriber() -> Result<(), SubscriberInstallError> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .try_init()
        .map_err(|_| SubscriberInstallError::AlreadyInstalled)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_default_subscriber_is_callable() {
        // Test ordering is non-deterministic; both outcomes are
        // structurally valid. The contract is "function exists and
        // returns the right type" — we don't assert which one because
        // a sibling test may have already won the global-install race.
        match install_default_subscriber() {
            Ok(()) => {}
            Err(SubscriberInstallError::AlreadyInstalled) => {}
        }
    }
}
