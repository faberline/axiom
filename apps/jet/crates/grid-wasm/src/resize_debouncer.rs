//! Pure ResizeObserver → RAF debouncer state machine. Slice 4cc (#1747).
//!
//! Why this module exists: a browser `ResizeObserver` fires dozens of
//! times per second during a pane drag. The renderer's `on_resize`
//! path is expensive — it reconfigures the swap chain, which on most
//! drivers means destroying + recreating surface textures. Calling it
//! per-event tears the frame and wastes GPU work. The fix is to
//! coalesce all events that arrive within the same animation-frame
//! window into a single most-recent observation, then apply that
//! once per RAF.
//!
//! Separating the coalescing logic from the `web_sys` plumbing keeps
//! it host-testable: this module compiles on every target (including
//! the host where `cargo test` runs), and the invariant the live
//! browser code relies on — "multiple observes between flushes
//! collapse to one take" — is asserted directly. The wasm-side
//! `RendererHandle::install_resize_observer` is the thin shim that
//! pushes browser-sourced events into here and drains it on RAF.

/// Coalesces resize events into at-most-one-per-frame state. Pure
/// state machine — no `web_sys` dependency so host tests can
/// exercise the debounce invariant without a browser.
///
/// @spec apps/jet/crates/grid-wasm/docs/canvas-resize-observer-slice-4cc.md#interface
/// @issue #1747
#[derive(Debug, Default)]
pub struct ResizeDebouncer {
    /// `Some((logical_w, logical_h, dpr))` if at least one
    /// observation has landed since the last `take_pending`. `None`
    /// means the next RAF need not call `on_resize`.
    pending: Option<(u32, u32, f32)>,
}

impl ResizeDebouncer {
    /// New, empty debouncer.
    ///
    /// @spec apps/jet/crates/grid-wasm/docs/canvas-resize-observer-slice-4cc.md#interface
    /// @issue #1747
    pub fn new() -> Self {
        Self { pending: None }
    }

    /// Record a new pending resize. Overwrites any prior pending
    /// values — only the most recent observation wins, which is the
    /// correct semantics for a debounce (the user cares about the
    /// final size after a drag, not the intermediate values).
    ///
    /// Clamps `logical_w` and `logical_h` to at least 1 (wgpu rejects
    /// zero-sized surfaces — the minimized-window case maps to a
    /// 1×1 placeholder). Clamps `dpr` to at least 1.0 when the
    /// caller passes a non-positive value (some browsers report
    /// `0` or `NaN` for `devicePixelRatio` during mid-resize
    /// transients).
    ///
    /// @spec apps/jet/crates/grid-wasm/docs/canvas-resize-observer-slice-4cc.md#interface
    /// @issue #1747
    pub fn observe(&mut self, logical_w: u32, logical_h: u32, dpr: f32) {
        let safe_w = logical_w.max(1);
        let safe_h = logical_h.max(1);
        let safe_dpr = if dpr.is_finite() && dpr > 0.0 {
            dpr
        } else {
            1.0
        };
        self.pending = Some((safe_w, safe_h, safe_dpr));
    }

    /// Drain the pending event, if any. Returns `Some((w, h, dpr))`
    /// when an observation is waiting, `None` otherwise. Clears the
    /// state — subsequent calls before a fresh `observe` return
    /// `None`.
    ///
    /// @spec apps/jet/crates/grid-wasm/docs/canvas-resize-observer-slice-4cc.md#interface
    /// @issue #1747
    pub fn take_pending(&mut self) -> Option<(u32, u32, f32)> {
        self.pending.take()
    }

    /// `true` iff there is an unconsumed observation. The wasm
    /// bridge uses this to avoid scheduling redundant
    /// `requestAnimationFrame` callbacks: if a RAF is already
    /// in-flight (which it is whenever `has_pending` was true at
    /// the previous observe), don't schedule another.
    ///
    /// @spec apps/jet/crates/grid-wasm/docs/canvas-resize-observer-slice-4cc.md#interface
    /// @issue #1747
    pub fn has_pending(&self) -> bool {
        self.pending.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_debouncer_has_no_pending() {
        let d = ResizeDebouncer::new();
        assert!(!d.has_pending());
        // take_pending on an empty debouncer must not panic.
        let mut d = d;
        assert_eq!(d.take_pending(), None);
    }

    #[test]
    fn observe_then_take_pending_returns_latest() {
        // AC anchor: a single observe + take round-trips the values
        // (post-clamping).
        let mut d = ResizeDebouncer::new();
        d.observe(800, 600, 2.0);
        assert!(d.has_pending());
        assert_eq!(d.take_pending(), Some((800, 600, 2.0)));
        // After take, the debouncer is empty again.
        assert!(!d.has_pending());
        assert_eq!(d.take_pending(), None);
    }

    #[test]
    fn multiple_observes_collapse_to_one_take() {
        // AC anchor: "Debounce to RAF" — multiple observe calls
        // between flushes must yield exactly one take with the
        // most-recent values. This is the correctness invariant
        // the live ResizeObserver depends on.
        let mut d = ResizeDebouncer::new();
        d.observe(100, 100, 1.0);
        d.observe(200, 200, 1.5);
        d.observe(300, 250, 2.0);
        assert_eq!(d.take_pending(), Some((300, 250, 2.0)));
        // Only ONE take's worth of state — the prior observes were
        // overwritten, not queued.
        assert_eq!(d.take_pending(), None);
    }

    #[test]
    fn observe_clamps_zero_dimensions_to_one() {
        // wgpu rejects zero-sized surfaces; the debouncer clamps
        // before pushing to avoid surfacing the panic.
        let mut d = ResizeDebouncer::new();
        d.observe(0, 0, 1.0);
        assert_eq!(d.take_pending(), Some((1, 1, 1.0)));
    }

    #[test]
    fn observe_clamps_invalid_dpr_to_one() {
        // Some browsers report `0` or `NaN` for devicePixelRatio
        // during mid-resize transients; the debouncer normalizes to
        // 1.0 so the downstream renderer never sees a bogus dpr.
        let mut d = ResizeDebouncer::new();
        d.observe(100, 100, 0.0);
        assert_eq!(d.take_pending().map(|t| t.2), Some(1.0));

        d.observe(100, 100, -2.0);
        assert_eq!(d.take_pending().map(|t| t.2), Some(1.0));

        d.observe(100, 100, f32::NAN);
        assert_eq!(d.take_pending().map(|t| t.2), Some(1.0));

        d.observe(100, 100, f32::INFINITY);
        assert_eq!(d.take_pending().map(|t| t.2), Some(1.0));
    }

    #[test]
    fn has_pending_tracks_observe_take_lifecycle() {
        let mut d = ResizeDebouncer::new();
        assert!(!d.has_pending());
        d.observe(100, 100, 1.0);
        assert!(d.has_pending());
        // Has-pending stays true across additional observes.
        d.observe(200, 200, 2.0);
        assert!(d.has_pending());
        let _ = d.take_pending();
        assert!(!d.has_pending());
    }
}
