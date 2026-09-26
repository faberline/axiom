# RAF-driven frame loop on the JS side — Slice 4f

> **Issue**: #1724
> **Parent epic**: #1254 (WebGPU-React renderer)
> **Depends on**: #1719–#1723 (Slices 4a–4e — renderer + render_frame).
> **Home crate**: `jet-grid-wasm` (JS-facing wasm-bindgen surface).
> **Status**: in-flight

## Problem

Slice 4e shipped `WebGpuRenderer::render_frame(&[CellInstance])` — the
spine of the per-frame render pass. But it has no driver: nothing calls
it on each animation frame, nothing pauses when the viewport is
off-screen, nothing cancels the loop on unmount.

This slice adds that driver — a `WasmView` wasm-bindgen component that
JS instantiates against a `<canvas>`, then calls `.start()` on. The
component owns a `requestAnimationFrame` loop, queries its data plane on
each frame, builds the visible-cell `CellInstance` vec, and drives
`render_frame`. It also pauses the loop when an `IntersectionObserver`
reports the canvas off-screen — important for grids embedded in long
scrollable pages so we don't burn GPU on invisible content.

## Scope

In:

- Pure-Rust **state machine** for the frame loop with three states:
  `Idle`, `RunningVisible`, `RunningHidden`. Transitions exposed via
  `start()`, `stop()`, `set_visible(bool)`. Idempotent — repeated
  `start()` on a running view is a no-op; `stop()` on `Idle` is a
  no-op. This state machine is host-testable without wasm.
- `tick_count()` accessor — total `tick()` calls observed by the state
  machine. Lets host tests assert "stop cancels future ticks" without
  needing to introspect RAF handles.
- `should_render()` predicate — `true` iff the view is in
  `RunningVisible`. The RAF callback consults this before issuing
  `render_frame`; it's also testable on the host.
- **wasm32 wiring** (gated `#[cfg(target_arch = "wasm32")]`):
  - `WasmView::new(canvas: web_sys::HtmlCanvasElement)` constructor,
    `#[wasm_bindgen]`-exposed.
  - `start()` registers a RAF closure that calls back into the state
    machine and (if `should_render`) renders the current frame.
  - `stop()` cancels the outstanding RAF handle via
    `cancelAnimationFrame`.
  - `set_observer(observer: web_sys::IntersectionObserver)` /
    `notify_intersect(is_visible: bool)` — the JS side wires its
    observer; on each callback, JS calls `notify_intersect` which
    routes into `set_visible`.
- Module-level doc on **why the loop pauses instead of stopping** on
  off-screen: keeping the closure alive avoids the alloc + observer
  re-bind cost when the canvas scrolls back into view.

Out:

- Actually building a `CellInstance` vec from the data plane. That
  belongs to the data plane / view-model slice — this slice exposes a
  `set_render_callback(Fn() -> Vec<CellInstance>)` hook and lets the
  caller (or the next slice) supply the data. For this slice the
  callback returns `vec![]` by default.
- Multi-canvas support — one `WasmView` ↔ one `WebGpuRenderer` ↔ one
  canvas. The state machine has no `view_id` because there is no fanout.
- Backpressure / frame-skipping when the GPU is slow. Each RAF tick
  issues exactly one `render_frame`; if it returns `Outdated`, the
  state machine logs and the next tick retries.
- Frame budget telemetry / profiler hooks. Later slice.

## Interface

```rust
// apps/jet/crates/grid-wasm/src/frame_loop.rs

/// The lifecycle state of the RAF-driven frame loop.
///
/// @issue #1724
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LoopState {
    /// No RAF is scheduled. `start()` transitions to `RunningVisible`.
    Idle,
    /// Canvas is on-screen; each RAF tick renders.
    RunningVisible,
    /// Canvas is off-screen; RAF still fires but the renderer is skipped.
    /// `set_visible(true)` transitions back to `RunningVisible`.
    RunningHidden,
}

/// Host-testable state machine for the RAF-driven frame loop.
///
/// Wraps the lifecycle transitions in a side-effect-free struct so
/// `cargo test -p jet-grid-wasm` (host target) can exercise the full
/// transition matrix without a browser.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/raf-frame-loop-slice-4f.md#interface
/// @issue #1724
pub struct LoopController {
    state: LoopState,
    tick_count: u64,
}

impl LoopController {
    pub fn new() -> Self;
    pub fn state(&self) -> LoopState;
    pub fn tick_count(&self) -> u64;
    pub fn should_render(&self) -> bool;          // Idle | RunningHidden -> false
    pub fn start(&mut self);                       // Idle/Hidden/Visible -> Visible
    pub fn stop(&mut self);                        // any -> Idle, tick_count preserved
    pub fn set_visible(&mut self, visible: bool); // Visible<->Hidden; Idle stays Idle
    pub fn tick(&mut self) -> bool;                // increments counter, returns should_render()
}
```

`tick()` is the side-effect-free probe the wasm32 RAF closure calls.
Returning `should_render()` post-increment is what tells the closure
whether to actually invoke `render_frame`.

## Acceptance criteria

- [x] `WasmView` component uses `requestAnimationFrame` internally
      (gated `#[cfg(target_arch = "wasm32")]`; verified by inspection +
      wasm-pack build smoke test downstream).
- [x] On each frame: query data plane → build `CellInstance` vec → call
      `render_frame`. The "build CellInstance vec" step is delegated to
      a caller-supplied callback (`set_render_callback`); this slice
      validates the wiring, not the data-plane query (out of scope —
      see Scope).
- [x] Cancel RAF on unmount — `stop()` calls
      `Window::cancel_animation_frame` with the stored handle.
- [x] Pause RAF when viewport not visible (intersection observer):
      `notify_intersect(false)` transitions to `RunningHidden`; the RAF
      closure still fires but `tick()` returns `false`, skipping the
      render call.
- [x] `cargo test -p jet-grid-wasm` passes (host-target tests cover
      every transition; wasm-only paths compile only).
- [x] Module-level docs explain WHY (pause vs stop trade-off; one
      view ↔ one renderer cardinality).

## Reference context

- `apps/jet/crates/grid-render-webgpu/src/lib.rs` — Slice 4e
  `WebGpuRenderer::render_frame` this loop drives.
- `apps/jet/crates/grid-render-webgpu/src/cell_rect.rs` — `CellInstance`
  the (deferred) data-plane callback constructs.
- `apps/jet/crates/grid-wasm/src/api.rs` — existing wasm-bindgen surface
  (`SpreadsheetEngine`). New module `frame_loop.rs` is registered
  alongside.
- web-sys docs: `Window::request_animation_frame(&Closure<dyn FnMut(f64)>)`
  returns an `i32` handle; pass it back to `cancel_animation_frame` on
  stop. `IntersectionObserver` is wired entirely on the JS side; Rust
  only sees the `notify_intersect(bool)` callback the JS handler invokes.
