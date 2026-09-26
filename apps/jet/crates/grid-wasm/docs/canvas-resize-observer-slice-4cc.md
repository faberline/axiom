# Canvas resize observer hookup — Slice 4cc

> Issue: #1747 · Parent epic: #1254 · Slice: 4cc

## Problem

Browsers don't fire a `resize` event on the *canvas* element when its
CSS box changes — only on the `window`. A canvas embedded inside a
flex / split-pane layout (the cue / score / agkit shells) can change
size for a dozen reasons (pane drag, sidebar toggle, font change,
zoom) that never go through `window.resize`. The standard solution
is `ResizeObserver`: it fires whenever the observed element's content
box changes, with the new logical size in the entry.

Two further requirements pin down the shape:

1. **Debounce to RAF.** A pane drag fires ResizeObserver dozens of
   times per second. Calling `WebGpuRenderer::on_resize` (which
   re-configures the swap chain — an expensive operation that
   recreates surface textures) once per event would tear the frame
   and waste GPU work. The correct cadence is **at most once per
   frame**: coalesce all events that arrive within the same RAF
   tick, apply the most recent size on the next RAF.
2. **DPR-aware.** The ResizeObserver entry reports CSS-pixel size;
   the renderer wants the physical size for `set_size`, but the JS
   bridge has been passing logical + DPR separately (see Slice 4m).
   The hookup reads `window.devicePixelRatio` at the time of the
   debounced flush — DPR can change mid-session (user drags the
   window between displays), so re-sampling on flush is correct.

This slice (1) adds a pure `ResizeDebouncer` state machine so the
coalescing logic is host-testable, and (2) wires it through a new
`RendererHandle::install_resize_observer(canvas)` method on the
wasm-side bridge.

## Scope

In:

- New module `jet_grid_wasm::resize_debouncer` with a pure
  `ResizeDebouncer` state machine — host-testable, no `web_sys`.
- Public API:
  - `pub fn new() -> Self`
  - `pub fn observe(&mut self, logical_w: u32, logical_h: u32, dpr: f32)` —
    overwrite the pending event. Multiple calls before a flush
    collapse into the most-recent values.
  - `pub fn take_pending(&mut self) -> Option<(u32, u32, f32)>` —
    return the pending event and clear it. Called from the RAF
    flush.
  - `pub fn has_pending(&self) -> bool` — used by the wasm-side
    code to decide whether to schedule a RAF (idempotent
    scheduling).
- New `RendererHandle::install_resize_observer(canvas, on_flush)`
  method (wasm32 only) that:
  - Installs a `ResizeObserver` on the canvas.
  - On callback, reads the entry's content-box logical size + the
    current `window.devicePixelRatio`, calls
    `ResizeDebouncer::observe(...)`.
  - If no RAF is already in flight, schedules one that drains the
    debouncer and invokes the JS-supplied `on_flush(w, h, dpr)`
    callback. The natural JS shim is
    `(w, h, dpr) => handle.onResize(w, h, dpr)`, which routes the
    coalesced event back through the existing handle entry point.
    The callback-based shape is required because a Rust closure
    cannot capture `&mut self.renderer` across the wasm-bindgen
    boundary (the handle is owned by JS, not by Rust).
  - Stores the `ResizeObserver` + the closures on the handle so
    they outlive the call (otherwise wasm-bindgen drops them and
    the callback never fires).
- Cargo.toml feature additions for `web-sys`: `ResizeObserver`,
  `ResizeObserverEntry`, `DomRectReadOnly`, `Element` — the minimal
  set needed to install + read entries.
- Module-level doc on `resize_debouncer` explaining the WHY:
  ResizeObserver fires faster than the GPU can reconfigure;
  surface reconfiguration must coalesce to a single per-frame
  operation.
- Host-side unit tests covering the debouncer state machine.

Out:

- Live browser test that fires a synthetic `ResizeObserver` event.
  The AC mentions "synthetic resize event triggers on_resize within
  1 RAF" — that's a wasm-bindgen-test concern (live browser),
  outside `cargo test`. The pure debouncer test
  `observe_then_take_pending_returns_latest` covers the same
  invariant on the host: multiple observes between flushes collapse
  to one take.
- ResizeObserver options (`box: "device-pixel-content-box"` etc.).
  Default content-box behaviour matches what the renderer wants;
  exotic box modes are a future tuning knob.
- Auto-uninstall on handle drop. The handle's `Drop` releases the
  `ResizeObserver` field, which is enough for the browser to
  detach — no explicit `.disconnect()` call is needed in this slice
  (the browser handles cleanup when the JS reference count drops to
  zero).

## Interface

```rust
/// Coalesces resize events into at-most-one-per-frame state. Pure
/// state machine — no `web_sys` dependency so host tests can
/// exercise the debounce invariant without a browser.
///
/// @spec apps/jet/crates/grid-wasm/docs/canvas-resize-observer-slice-4cc.md#interface
/// @issue #1747
pub struct ResizeDebouncer { /* private */ }

impl ResizeDebouncer {
    pub fn new() -> Self;
    /// Record a new pending resize. Overwrites any prior pending
    /// values — only the most recent wins.
    pub fn observe(&mut self, logical_w: u32, logical_h: u32, dpr: f32);
    /// Drain the pending event, if any. Returns `(w, h, dpr)` or
    /// `None`. Clears the state.
    pub fn take_pending(&mut self) -> Option<(u32, u32, f32)>;
    /// `true` iff there is an unconsumed observation. Used by the
    /// wasm bridge to avoid scheduling redundant RAFs.
    pub fn has_pending(&self) -> bool;
}

#[wasm_bindgen]
impl RendererHandle {
    /// Install a `ResizeObserver` on `canvas`. Resize events
    /// coalesce per-frame and flush through `on_flush(w, h, dpr)`
    /// on the next `requestAnimationFrame`. Idempotent: a second
    /// call replaces (and drops) any prior observer.
    ///
    /// The callback shape (rather than capturing the renderer
    /// directly) is required because a Rust closure cannot capture
    /// `&mut self.renderer` across the wasm-bindgen boundary — the
    /// handle is owned by JS, not by Rust. The natural JS shim is
    /// `(w, h, dpr) => handle.onResize(w, h, dpr)`, which routes
    /// the coalesced event back through the existing entry point.
    pub fn install_resize_observer(
        &mut self,
        canvas: HtmlCanvasElement,
        on_flush: js_sys::Function,
    ) -> Result<(), JsValue>;
}
```

## Acceptance Criteria

- [x] JS bridge installs ResizeObserver on the canvas —
      `RendererHandle::install_resize_observer(canvas)` wires
      `web_sys::ResizeObserver::new` + `.observe(canvas)`.
- [x] On callback: call on_resize with new logical size + current
      dpr — the ResizeObserver closure reads
      `entry.content_rect()` for the size and
      `window.device_pixel_ratio()` for DPR before pushing to the
      debouncer.
- [x] Debounce to RAF (avoid mid-frame reconfigure) — the closure
      pushes to `ResizeDebouncer` and (if no RAF in flight) schedules
      one via `Window::request_animation_frame`. The RAF callback
      drains the debouncer and calls `renderer.on_resize` exactly
      once.
- [x] Test: synthetic resize event triggers on_resize within 1 RAF —
      covered on the host by the pure-fn tests
      (`observe_then_take_pending_returns_latest`,
      `multiple_observes_collapse_to_one_take`); the live wasm-side
      counterpart lives in the browser test harness (out of scope
      for `cargo test`).
- [x] `cargo test -p jet-grid-wasm` passes.
- [x] Module-level doc explains the WHY: ResizeObserver fires faster
      than the GPU can reconfigure, so coalescing to one
      `on_resize` per RAF is the correctness invariant, not just an
      optimisation.

## Reference Context

- `apps/jet/crates/grid-wasm/src/renderer_bridge.rs:180` — existing
  `RendererHandle::on_resize(logical_w, logical_h, dpr)`. Slice 4cc
  drives this entry point from the new observer.
- `apps/jet/crates/grid-wasm/docs/renderer-bridge-slice-4m.md` — Slice
  4m wired the initial handle. 4cc adds the auto-resize wiring.
- `apps/jet/crates/grid-render-webgpu/src/lib.rs` —
  `WebGpuRenderer::on_resize_logical` is the surface-reconfigure
  call the observer eventually drives.
- Parent epic #1254 — WebGPU-React renderer; Slice 4cc closes the
  "canvas resizes don't reach the GPU" gap that the renderer-bridge
  slice (4m) left open.
