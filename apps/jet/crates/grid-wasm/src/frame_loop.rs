//! RAF-driven frame loop scaffolding for the JS-facing grid view.
//!
//! Why this lives here: the jet-grid-render-webgpu crate owns the
//! per-frame GPU work (`WebGpuRenderer::render_frame`), but nothing
//! schedules calls into it. The browser side needs a tiny state machine
//! that ties `requestAnimationFrame` + `IntersectionObserver` to that
//! render call so off-screen grids don't burn GPU and unmounted grids
//! don't leak RAF callbacks.
//!
//! Why the loop *pauses* instead of *stops* on off-screen: an embedded
//! grid scrolling in and out of a long page can re-enter visibility in
//! ~tens of milliseconds. Tearing down the RAF closure + observer on
//! every scroll-out (then re-binding on every scroll-in) burns more
//! allocator + JS-bridge churn than the cost of a no-op RAF tick. So
//! the running states split into `RunningVisible` (RAF fires, render
//! happens) and `RunningHidden` (RAF fires, render is skipped); only
//! `stop()` (unmount, explicit teardown) tears the closure down.
//!
//! Cardinality invariant: one `WasmView` ↔ one `WebGpuRenderer` ↔ one
//! `<canvas>`. The state machine has no `view_id` because there is no
//! fanout — if multiple grids exist on a page, each owns its own
//! `WasmView`.

use wasm_bindgen::prelude::*;

/// The lifecycle state of the RAF-driven frame loop.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/raf-frame-loop-slice-4f.md#interface
/// @issue #1724
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LoopState {
    /// No RAF is scheduled. `start()` transitions to `RunningVisible`.
    Idle,
    /// Canvas is on-screen; each RAF tick renders.
    RunningVisible,
    /// Canvas is off-screen; RAF still fires but the renderer is
    /// skipped. `set_visible(true)` transitions back to
    /// `RunningVisible`.
    RunningHidden,
}

/// Host-testable state machine for the RAF-driven frame loop.
///
/// Side-effect-free: every transition mutates only this struct. The
/// wasm32 wiring in `WasmView` calls into the controller from inside
/// its RAF closure, and `should_render()` decides whether to issue the
/// frame.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/raf-frame-loop-slice-4f.md#interface
/// @issue #1724
#[derive(Debug)]
pub struct LoopController {
    state: LoopState,
    tick_count: u64,
}

impl Default for LoopController {
    fn default() -> Self {
        Self::new()
    }
}

impl LoopController {
    /// Construct a controller in the [`LoopState::Idle`] state.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/raf-frame-loop-slice-4f.md#interface
    /// @issue #1724
    pub fn new() -> Self {
        Self {
            state: LoopState::Idle,
            tick_count: 0,
        }
    }

    /// Current lifecycle state.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/raf-frame-loop-slice-4f.md#interface
    /// @issue #1724
    pub fn state(&self) -> LoopState {
        self.state
    }

    /// Total `tick()` calls observed since construction. Not reset by
    /// `stop()` — useful for tests that want to assert "no further
    /// ticks happen after stop".
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/raf-frame-loop-slice-4f.md#interface
    /// @issue #1724
    pub fn tick_count(&self) -> u64 {
        self.tick_count
    }

    /// `true` iff the next render-pass call should actually run.
    /// Equivalent to `state == RunningVisible`.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/raf-frame-loop-slice-4f.md#interface
    /// @issue #1724
    pub fn should_render(&self) -> bool {
        matches!(self.state, LoopState::RunningVisible)
    }

    /// Move the controller into a running state. Idempotent — calling
    /// `start()` on an already-running controller is a no-op
    /// (preserves the visible/hidden distinction). On `Idle`,
    /// transitions to `RunningVisible` (the safe default — visibility
    /// is corrected by the first observer callback on the JS side).
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/raf-frame-loop-slice-4f.md#interface
    /// @issue #1724
    pub fn start(&mut self) {
        if let LoopState::Idle = self.state {
            self.state = LoopState::RunningVisible;
        }
    }

    /// Tear the loop down. Idempotent. `tick_count` is preserved.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/raf-frame-loop-slice-4f.md#interface
    /// @issue #1724
    pub fn stop(&mut self) {
        self.state = LoopState::Idle;
    }

    /// Update the visibility flag from the IntersectionObserver
    /// callback. `Idle` is preserved (visibility changes while
    /// unmounted are meaningless); the running states toggle between
    /// `RunningVisible` and `RunningHidden`.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/raf-frame-loop-slice-4f.md#interface
    /// @issue #1724
    pub fn set_visible(&mut self, visible: bool) {
        self.state = match (self.state, visible) {
            (LoopState::Idle, _) => LoopState::Idle,
            (_, true) => LoopState::RunningVisible,
            (_, false) => LoopState::RunningHidden,
        };
    }

    /// Side-effect-free per-tick probe. Increments `tick_count` and
    /// returns whether the caller should actually issue a render call.
    /// In `Idle` the counter does NOT advance — there is no live RAF
    /// to source ticks in that state.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/raf-frame-loop-slice-4f.md#interface
    /// @issue #1724
    pub fn tick(&mut self) -> bool {
        if let LoopState::Idle = self.state {
            return false;
        }
        self.tick_count += 1;
        self.should_render()
    }
}

// ---------------------------------------------------------------------------
// wasm32-only wiring. The host build sees only `LoopController` above; the
// wasm32 build adds the JS-facing `WasmView` wrapper that owns the RAF
// closure + cancellation handle. Gated so `cargo test -p jet-grid-wasm`
// on the host target exercises the state machine without dragging in
// browser APIs.
// ---------------------------------------------------------------------------

/// JS-facing wrapper around [`LoopController`] that owns the RAF closure
/// and cancellation handle. Constructed from a `<canvas>` on the JS side;
/// callers wire IntersectionObserver themselves and invoke
/// `notify_intersect(bool)` per observer callback.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/raf-frame-loop-slice-4f.md#interface
/// @issue #1724
#[wasm_bindgen]
pub struct WasmView {
    controller: LoopController,
    #[cfg(target_arch = "wasm32")]
    canvas: web_sys::HtmlCanvasElement,
    /// Stored RAF handle so `stop()` can cancel it. Only meaningful on
    /// wasm32 — the field is gated to avoid a host-target dead-code
    /// warning, since there is no host constructor for `WasmView`.
    #[cfg(target_arch = "wasm32")]
    raf_handle: Option<i32>,
    /// Anchor for the active RAF closure. Re-bound every tick (since
    /// `request_animation_frame` is one-shot) so the closure can
    /// schedule its own successor.
    #[cfg(target_arch = "wasm32")]
    raf_closure: Option<wasm_bindgen::closure::Closure<dyn FnMut(f64)>>,
}

#[wasm_bindgen]
impl WasmView {
    /// Construct a `WasmView` bound to `canvas`. On wasm32, the canvas
    /// reference is retained so `start()` can resolve the `window` and
    /// schedule the RAF.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/raf-frame-loop-slice-4f.md#interface
    /// @issue #1724
    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen(constructor)]
    pub fn new(canvas: web_sys::HtmlCanvasElement) -> Self {
        Self {
            controller: LoopController::new(),
            canvas,
            raf_handle: None,
            raf_closure: None,
        }
    }

    /// Current lifecycle state.
    ///
    /// @issue #1724
    #[wasm_bindgen(getter, js_name = isRunning)]
    pub fn is_running(&self) -> bool {
        !matches!(self.controller.state(), LoopState::Idle)
    }

    /// Update the visibility flag from the JS-side IntersectionObserver
    /// handler. `true` → resume rendering; `false` → keep the RAF but
    /// skip the render call.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/raf-frame-loop-slice-4f.md#interface
    /// @issue #1724
    #[wasm_bindgen(js_name = notifyIntersect)]
    pub fn notify_intersect(&mut self, is_visible: bool) {
        self.controller.set_visible(is_visible);
    }

    /// Start the RAF loop. Idempotent. The first call transitions
    /// `Idle -> RunningVisible` and schedules the initial RAF; later
    /// calls while running are no-ops.
    ///
    /// On non-wasm32 targets this only mutates the state machine —
    /// scheduling is unavailable without a browser.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/raf-frame-loop-slice-4f.md#interface
    /// @issue #1724
    pub fn start(&mut self) {
        let was_idle = matches!(self.controller.state(), LoopState::Idle);
        self.controller.start();
        if was_idle {
            #[cfg(target_arch = "wasm32")]
            self.schedule_next_frame();
        }
    }

    /// Stop the RAF loop and release the closure. Idempotent.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/raf-frame-loop-slice-4f.md#interface
    /// @issue #1724
    pub fn stop(&mut self) {
        self.controller.stop();
        #[cfg(target_arch = "wasm32")]
        {
            if let (Some(handle), Some(window)) = (self.raf_handle.take(), web_sys::window()) {
                window.cancel_animation_frame(handle).ok();
            }
            // Dropping the closure releases the JS function reference
            // and (under wasm-bindgen) the Rust-side environment.
            self.raf_closure = None;
        }
    }
}

#[cfg(target_arch = "wasm32")]
impl WasmView {
    /// Schedule the next RAF callback, rebinding a one-shot closure
    /// that calls back into this view via a raw pointer.
    ///
    /// Why the raw pointer dance: `request_animation_frame` consumes
    /// `&Closure<dyn FnMut(f64)>`, and the closure outlives this
    /// function call. We can't move `&mut self` into it, and a
    /// `Rc<RefCell<_>>` per WasmView would force every JS-facing
    /// method to go through the borrow check. The pointer is only
    /// dereferenced from the RAF callback, which fires synchronously
    /// from the browser on the same thread; `stop()` cancels the RAF
    /// + drops the closure before the WasmView itself drops, so the
    /// pointer can never outlive the target.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/raf-frame-loop-slice-4f.md#interface
    fn schedule_next_frame(&mut self) {
        let Some(window) = web_sys::window() else {
            return;
        };
        let self_ptr: *mut WasmView = self;
        let closure = wasm_bindgen::closure::Closure::wrap(Box::new(move |_now: f64| {
            // SAFETY: the closure is dropped in `stop()` before the
            // WasmView itself goes out of scope; the browser dispatches
            // RAF callbacks synchronously on the main thread so we
            // never race with `stop()`.
            let view = unsafe { &mut *self_ptr };
            if view.controller.tick() {
                // TODO(slice-4g): drive `WebGpuRenderer::render_frame`
                // through the caller-supplied render callback here.
                // This slice's AC is "the loop fires + skips when
                // hidden"; the actual render call lands when the
                // renderer + data-plane integration slice arrives.
            }
            if !matches!(view.controller.state(), LoopState::Idle) {
                view.schedule_next_frame();
            }
        }) as Box<dyn FnMut(f64)>);

        match window.request_animation_frame(closure.as_ref().unchecked_ref()) {
            Ok(handle) => {
                self.raf_handle = Some(handle);
                self.raf_closure = Some(closure);
                let _ = &self.canvas; // suppress unused warning until 4g wires it
            }
            Err(_) => {
                // RAF unavailable (no rAF-capable window). Drop back to
                // Idle so the next start() can retry cleanly.
                self.controller.stop();
                self.raf_handle = None;
                self.raf_closure = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_is_idle_with_zero_ticks() {
        let c = LoopController::new();
        assert_eq!(c.state(), LoopState::Idle);
        assert_eq!(c.tick_count(), 0);
        assert!(!c.should_render());
    }

    #[test]
    fn start_transitions_idle_to_running_visible() {
        let mut c = LoopController::new();
        c.start();
        assert_eq!(c.state(), LoopState::RunningVisible);
        assert!(c.should_render());
    }

    #[test]
    fn start_is_idempotent_on_running_visible() {
        let mut c = LoopController::new();
        c.start();
        c.start();
        assert_eq!(c.state(), LoopState::RunningVisible);
    }

    #[test]
    fn start_does_not_change_running_hidden() {
        // Idempotency invariant: while running, repeated start() must
        // preserve visibility — otherwise an explicit hide could be
        // silently undone by a redundant start() call.
        let mut c = LoopController::new();
        c.start();
        c.set_visible(false);
        c.start();
        assert_eq!(c.state(), LoopState::RunningHidden);
    }

    #[test]
    fn set_visible_toggles_running_state() {
        let mut c = LoopController::new();
        c.start();
        c.set_visible(false);
        assert_eq!(c.state(), LoopState::RunningHidden);
        assert!(!c.should_render());
        c.set_visible(true);
        assert_eq!(c.state(), LoopState::RunningVisible);
        assert!(c.should_render());
    }

    #[test]
    fn set_visible_is_ignored_while_idle() {
        // Visibility changes on an unmounted view are meaningless and
        // must not silently re-arm the loop.
        let mut c = LoopController::new();
        c.set_visible(true);
        assert_eq!(c.state(), LoopState::Idle);
        c.set_visible(false);
        assert_eq!(c.state(), LoopState::Idle);
    }

    #[test]
    fn stop_returns_to_idle() {
        let mut c = LoopController::new();
        c.start();
        c.stop();
        assert_eq!(c.state(), LoopState::Idle);
        c.stop(); // idempotent
        assert_eq!(c.state(), LoopState::Idle);
    }

    #[test]
    fn tick_advances_counter_only_while_running() {
        let mut c = LoopController::new();
        assert!(!c.tick());
        assert_eq!(c.tick_count(), 0, "Idle tick must not advance counter");

        c.start();
        assert!(c.tick());
        assert_eq!(c.tick_count(), 1);

        c.set_visible(false);
        assert!(!c.tick(), "hidden tick must return false");
        assert_eq!(c.tick_count(), 2, "hidden tick still advances counter");

        c.stop();
        assert!(!c.tick(), "stopped tick must return false");
        assert_eq!(c.tick_count(), 2, "stopped tick must not advance counter");
    }

    #[test]
    fn lifecycle_round_trip_preserves_counter() {
        // tick_count must survive stop() so tests / telemetry can
        // observe the count after teardown.
        let mut c = LoopController::new();
        c.start();
        c.tick();
        c.tick();
        c.stop();
        assert_eq!(c.tick_count(), 2);
        c.start();
        c.tick();
        assert_eq!(c.tick_count(), 3);
    }
}
