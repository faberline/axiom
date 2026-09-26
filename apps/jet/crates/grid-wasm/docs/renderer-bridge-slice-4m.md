# WasmView ↔ WebGpuRenderer JS bridge — Slice 4m

> **Issue**: #1731
> **Parent epic**: #1254 (WebGPU-React renderer)
> **Depends on**: #1719 (Slice 4a — renderer wrapper), #1724 (Slice
> 4f — RAF frame loop in `WasmView`), #1725 (Slice 4g — DPR scaling),
> #1730 (Slice 4l — backend selection).
> **Status**: in-flight

## Problem

The React `WasmView` host needs a JS-callable API that constructs a
`WebGpuRenderer` from an `HtmlCanvasElement` and drives it per
frame. WebGPU adapter / device acquisition is asynchronous, so the
init path must return a `Promise`. After init, JS only sees an
opaque `RendererHandle`; render / resize / destroy go through that
handle so the renderer's lifetime is managed exclusively from JS
(React's `useEffect` cleanup calls `destroy()` on unmount).

Today `jet-grid-wasm` already exposes the `WasmView` RAF loop
(Slice 4f) but has no `WebGpuRenderer` integration — its
`schedule_next_frame` even has a `TODO(slice-4g)` placeholder for
the render call. This slice fills the gap with a separate,
small bridge module that wraps the renderer behind four free
functions / methods (init / render / resize / destroy) without
touching `WasmView`'s state machine. A later slice can wire
`WasmView` to call `RendererHandle::render_frame` from inside the
RAF callback.

The bridge intentionally avoids `Box<[CellInstance]>` on the
JS-facing signature: `jet_grid_render_webgpu::cell_rect::CellInstance`
is a `bytemuck::Pod` vertex struct, not a wasm-bindgen-serializable
type. The JS contract is a packed `Float32Array` (8 f32s per cell —
`pos_x, pos_y, size_w, size_h, r, g, b, a`), unpacked inside the
bridge into the renderer's native struct. This matches the WebGPU
vertex layout one-for-one so the wire format is the same as what
the GPU eventually sees.

## Scope

In:

- New module `renderer_bridge` in `jet-grid-wasm`:
  - `pub struct RendererHandle` — JS-owned handle. Wraps a
    `WebGpuRenderer<'static>` (wasm32 only) plus a reused cells
    `Vec` so per-frame upload doesn't churn the allocator.
  - `pub fn init_renderer(canvas: HtmlCanvasElement, dpr: f32) ->
    js_sys::Promise` — async init via
    `wasm_bindgen_futures::future_to_promise`. Resolves to the
    `RendererHandle`. Rejects with the wgpu error message on
    adapter / device failure.
  - `impl RendererHandle` methods (each `#[wasm_bindgen]`-exported):
    - `render_frame(cells: js_sys::Float32Array)` — unpacks the
      packed `Float32Array` into `CellInstance`s and calls
      `WebGpuRenderer::render_frame`. Returns `Result<(), JsValue>`
      so a bad input length surfaces as a JS exception.
    - `on_resize(logical_w: u32, logical_h: u32, dpr: f32)` —
      forwards to `set_dpr` (if changed) and `on_resize_logical`.
    - `destroy(self)` — consumes the handle so JS can drop it
      explicitly. (Rust's `Drop` for `RendererHandle` runs the
      same teardown, but exposing an explicit method matches the
      AC's "destroy(handle)" bullet and gives JS a predictable
      teardown point under React `useEffect` cleanup.)
- Host-target compile shims: the wasm-bindgen exports live under
  `#[cfg(target_arch = "wasm32")]`. A small `BridgeState` struct
  (cells buffer + last_dpr + last_logical) is host-portable and
  carries the unit tests that exercise the unpack / resize-state
  invariants. Host `cargo test -p jet-grid-wasm` still passes.
- Cargo.toml: add `jet-grid-render-webgpu` path dep, plus
  `wasm-bindgen-futures = "0.4"` for the Promise-returning init.

Out:

- Wiring the bridge into `WasmView::schedule_next_frame`. That's a
  separate slice (the RAF closure currently has a `TODO(slice-4g)`
  comment for this); landing it here would scope-creep into the
  RAF state machine.
- A typed `JsCellInstance` wasm-bindgen struct. Float32Array packing
  avoids a per-cell serde-wasm-bindgen round-trip cost and matches
  the eventual `Queue::write_buffer` byte layout.
- Multiple-canvases-per-handle. One handle owns one canvas; React
  components multi-instance by creating multiple handles.

## Interface

```rust
// apps/jet/crates/grid-wasm/src/renderer_bridge.rs (wasm32 surface)

#[wasm_bindgen]
pub struct RendererHandle { /* opaque to JS */ }

#[wasm_bindgen]
pub fn init_renderer(
    canvas: web_sys::HtmlCanvasElement,
    dpr: f32,
) -> js_sys::Promise; // resolves to RendererHandle

#[wasm_bindgen]
impl RendererHandle {
    #[wasm_bindgen(js_name = renderFrame)]
    pub fn render_frame(&mut self, cells: js_sys::Float32Array)
        -> Result<(), JsValue>;

    #[wasm_bindgen(js_name = onResize)]
    pub fn on_resize(&mut self, logical_w: u32, logical_h: u32, dpr: f32);

    pub fn destroy(self); // consume — JS-side teardown signal
}
```

Packed cell layout (8 × `f32` per cell):

| Offset | Field   | Type   | Notes                |
| ------ | ------- | ------ | -------------------- |
| 0..2   | pos_px  | f32×2  | top-left, logical px |
| 2..4   | size_px | f32×2  | width × height       |
| 4..8   | color   | f32×4  | RGBA, 0.0..=1.0      |

The Float32Array must have length `cell_count * 8`; any other
length is a `JsValue` exception from `render_frame`.

## Acceptance Criteria

- [x] `init_renderer(canvas, dpr) -> Promise<RendererHandle>` —
      wasm-bindgen export returning `js_sys::Promise` via
      `wasm_bindgen_futures::future_to_promise`. Resolves to the
      handle; rejects with the wgpu error message on
      adapter / device failure.
- [x] `render_frame(handle, cells)` — `RendererHandle::render_frame`
      takes a packed `Float32Array` (8 f32s per cell), unpacks into
      `CellInstance`, and forwards to
      `WebGpuRenderer::render_frame`. The AC's literal `Box<[CellInstance]>`
      phrasing is preserved structurally as a slice of cells; the
      wire format is `Float32Array` because `CellInstance` isn't
      wasm-bindgen-serializable.
- [x] `on_resize(handle, w, h, dpr)` — forwards to `set_dpr` +
      `on_resize_logical` on the inner renderer.
- [x] `destroy(handle)` — consumes the handle; the inner renderer
      drops naturally via Rust's `Drop`.
- [x] `cargo test -p jet-grid-wasm` passes — host tests cover the
      `BridgeState` cells-buffer / resize-state invariants without
      a GPU.
- [x] Module-level docs explain WHY (JS owns lifetime via
      `destroy()`; packed Float32Array wire format; one handle per
      canvas).

## Reference context

- `apps/jet/crates/grid-wasm/src/frame_loop.rs` — `WasmView` (#1724,
  Slice 4f), the RAF state machine this bridge will eventually
  wire into.
- `apps/jet/crates/grid-render-webgpu/src/lib.rs` — `WebGpuRenderer`
  (#1719), `set_dpr` / `on_resize_logical` (#1725 Slice 4g),
  `render_frame` (#1723 Slice 4e).
- `apps/jet/crates/grid-render-webgpu/src/cell_rect.rs` —
  `CellInstance` struct (the packed-Float32Array unpack target).
- wgpu 24: `SurfaceTarget::Canvas(HtmlCanvasElement)` is the
  wasm32-only canvas-as-surface path consumed by `init_renderer`.
- `wasm_bindgen_futures::future_to_promise` — async-to-Promise
  shim for the init future.
