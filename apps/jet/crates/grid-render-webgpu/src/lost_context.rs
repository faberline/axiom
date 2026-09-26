//! Device-lost observation and recovery primitives.
//!
//! Why this module exists: a browser-owned WebGPU device can disappear
//! at any time — tab backgrounding, GPU driver crash (Windows TDR),
//! eGPU unplug. When that happens every subsequent wgpu call against
//! the dead device errors out, and the renderer must detect the loss,
//! tear down the dead state, request a fresh `(Device, Queue)` from
//! the same `Adapter`, and rebuild per-device resources.
//!
//! Invariant — surface survives device replacement: the [`wgpu::Surface`]
//! is bound to the OS window / canvas handle, NOT to the device, so it
//! is reusable across device swaps. The renderer keeps the same surface
//! and just calls `Surface::configure` with the new device. Anything
//! that *was* tied to the dead device (pipelines, shader modules, bind
//! groups, buffers) must be discarded — wgpu's safety model is that
//! cross-device handles are not valid.
//!
//! Invariant — single-take event: the [`LostContextStatus`] cell holds
//! the most recent [`DeviceLostEvent`]. Reading it via
//! [`LostContextStatus::take_event`] drains the cell so the React layer
//! observes each loss exactly once and never re-renders stale
//! "GPU restarting…" copy. The `lost` boolean stays `true` after the
//! event is taken — only a successful recovery clears it.
//!
//! Invariant — `Send + 'static` callback: `Device::set_device_lost_callback`
//! requires `impl Fn(DeviceLostReason, String) + Send + 'static`. That's
//! why the status cell is wrapped in `Arc<Mutex<...>>` and not a plain
//! `&mut bool` on the renderer.

use std::sync::{Arc, Mutex};

/// Snapshot of one device-lost notification. Pairs wgpu's
/// `DeviceLostReason` enum with the human-readable diagnostic the
/// driver supplied.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/lost-context-recovery-slice-4h.md#interface
/// @issue #1726
#[derive(Debug, Clone)]
pub struct DeviceLostEvent {
    /// Source of the loss — `Destroyed` if the caller explicitly
    /// dropped the device, `Unknown` for any other reason (driver
    /// crash, tab eviction, etc.).
    pub reason: wgpu::DeviceLostReason,
    /// Driver-supplied diagnostic message. Often empty in practice
    /// (the spec only requires the field exists); render-layer code
    /// should treat `""` as a normal value.
    pub message: String,
}

/// Shared status cell written by the `Device::set_device_lost_callback`
/// and read by the renderer. `Arc`-wrapped because the callback is
/// `Send + 'static` and outlives the borrow stack that registered it.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/lost-context-recovery-slice-4h.md#interface
/// @issue #1726
pub struct LostContextStatus {
    inner: Mutex<Inner>,
}

struct Inner {
    /// `true` once any device-lost callback has fired. Stays `true`
    /// until a successful recovery resets it.
    lost: bool,
    /// Most recent event seen by the callback. Drained by
    /// [`Self::take_event`]; the loss flag persists.
    event: Option<DeviceLostEvent>,
}

impl LostContextStatus {
    /// Build an empty status cell — no loss observed yet.
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(Inner {
                lost: false,
                event: None,
            }),
        })
    }

    /// Record a loss. Called only from the wgpu callback installed by
    /// [`install_callback`].
    fn record(&self, reason: wgpu::DeviceLostReason, message: String) {
        if let Ok(mut g) = self.inner.lock() {
            g.lost = true;
            // If a previous event hasn't been drained, the newer event
            // wins — the React layer sees the most recent failure.
            g.event = Some(DeviceLostEvent { reason, message });
        }
    }

    /// `true` if any device-lost callback has fired and no successful
    /// recovery has cleared the flag yet.
    pub fn is_lost(&self) -> bool {
        self.inner.lock().map(|g| g.lost).unwrap_or(false)
    }

    /// Single-take accessor: returns the most recent event AND clears
    /// the event slot (the loss flag itself is left set). Returns
    /// `None` if no event is pending.
    pub fn take_event(&self) -> Option<DeviceLostEvent> {
        self.inner.lock().ok().and_then(|mut g| g.event.take())
    }
}
// Note: a `clear` method intentionally does not exist. The renderer's
// recovery path allocates a brand-new [`LostContextStatus`] cell and
// installs the new device's callback against it, so the old cell —
// and any latent callbacks against the dead device — drops naturally.
// That keeps "is the renderer lost?" derivable from cell identity
// rather than a mutable flag.

/// Install `status` as the device-lost callback for `device`. The
/// callback is `Send + 'static` (wgpu's bound) and holds an `Arc`
/// clone of `status` so the status cell outlives the borrow stack
/// that registered it.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/lost-context-recovery-slice-4h.md#interface
/// @issue #1726
pub(crate) fn install_callback(device: &wgpu::Device, status: &Arc<LostContextStatus>) {
    let status = Arc::clone(status);
    device.set_device_lost_callback(move |reason, message| {
        status.record(reason, message);
    });
}

/// Failure modes for [`crate::WebGpuRenderer::try_recover`].
///
/// Variants name *which step* failed so the React layer can branch on
/// `request_device` vs `surface.configure` failures (driver-permanent
/// vs surface-target-gone).
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/lost-context-recovery-slice-4h.md#interface
/// @issue #1726
#[derive(Debug, thiserror::Error)]
pub enum RecoveryError {
    /// The adapter rejected `request_device` — driver state is
    /// unrecoverable. The React layer should fall back to a non-GPU
    /// rendering path or surface a "GPU unavailable" error to the user.
    #[error("adapter rejected request_device during recovery: {0}")]
    RequestDevice(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_status_is_not_lost() {
        let s = LostContextStatus::new();
        assert!(!s.is_lost());
        assert!(s.take_event().is_none());
    }

    #[test]
    fn record_sets_lost_and_provides_event() {
        let s = LostContextStatus::new();
        s.record(wgpu::DeviceLostReason::Unknown, "driver said no".into());
        assert!(s.is_lost());
        let ev = s.take_event().expect("event must be present");
        assert!(matches!(ev.reason, wgpu::DeviceLostReason::Unknown));
        assert_eq!(ev.message, "driver said no");
    }

    #[test]
    fn take_event_drains_but_keeps_lost_flag() {
        let s = LostContextStatus::new();
        s.record(wgpu::DeviceLostReason::Destroyed, "destroyed".into());
        let _ = s.take_event();
        // Single-take semantic: second take is None …
        assert!(s.take_event().is_none());
        // … but the loss flag persists (only `clear` resets it).
        assert!(s.is_lost());
    }

    #[test]
    fn newer_event_overwrites_older_undrained_event() {
        // If two callbacks fire before the React layer drains, the
        // most recent loss wins — the older one is best-effort
        // diagnostic and is replaced.
        let s = LostContextStatus::new();
        s.record(wgpu::DeviceLostReason::Unknown, "first".into());
        s.record(wgpu::DeviceLostReason::Destroyed, "second".into());
        let ev = s.take_event().expect("event present");
        assert!(matches!(ev.reason, wgpu::DeviceLostReason::Destroyed));
        assert_eq!(ev.message, "second");
    }

    #[test]
    fn recovery_error_display_names_step() {
        let e = RecoveryError::RequestDevice("driver dead".into());
        let s = e.to_string();
        assert!(s.contains("request_device"), "got: {s}");
        assert!(s.contains("driver dead"), "got: {s}");
    }

    /// Compile-time witness that `LostContextStatus` is `Send + Sync`.
    /// The wgpu callback registration requires `Send + 'static`; if
    /// either bound regresses, this test stops compiling.
    #[allow(dead_code)]
    fn status_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<LostContextStatus>();
        assert_send_sync::<Arc<LostContextStatus>>();
    }

    /// Compile-time witness that `DeviceLostEvent` is `Clone` —
    /// callers may want to log the event AND hand it to the React
    /// layer; cloning is cheap (one `String` clone).
    #[allow(dead_code)]
    fn event_is_clone(ev: DeviceLostEvent) -> DeviceLostEvent {
        ev.clone()
    }
}
