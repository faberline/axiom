# Cclab Grid Wasm

## Brief

Cclab Grid Wasm is the wasm-bindgen bridge that exposes the Rust spreadsheet
engine and grid renderer integration points to JavaScript.

It owns the JavaScript-facing `SpreadsheetEngine`, JSON/typed-array data
contracts, renderer bridge state, text-run planning, RAF visibility state
machine, resize debounce logic, and viewport packing helpers. The configured
host gate validates these Rust-side wasm-bindgen contracts; browser packaging,
WebGPU canvas rendering, RAF, and ResizeObserver journeys remain separate
browser/e2e coverage.

## Capabilities

A promise with no gate under it is not claimed.

### Capability Index

| Capability | Root WI | Notes |
|---|---:|---|
| JavaScript Spreadsheet Engine API | - | host tests cover the wasm-bindgen engine API and data contracts; browser package/e2e gate remains separate |
| WebGPU Renderer Bridge | - | host tests cover bridge invariants and text planning; browser WebGPU e2e gate remains separate |
| Browser Frame Loop And Resize Bridge | - | host tests cover state machines; live RAF/ResizeObserver integration gate remains separate |

### JavaScript Spreadsheet Engine API

Cclab Grid Wasm exposes the spreadsheet engine to JavaScript with workbook/cell
mutation, formula dependency recalculation, validation, formatting,
selection/history behavior, JSON serialization, and zero-copy viewport buffers.

- Root WI: none; this capability predates the tracker.
- Surfaces: WASM/JS API: `SpreadsheetEngine` - JavaScript-facing workbook,
  cell, formula, formatting, validation, selection, history, serialization, and
  viewport buffer API
- Gate — behavior: `cargo test -p jet-grid-wasm` - host-portable wasm-bindgen
  API invariants, formula recalculation, validation, formatting serialization,
  persistence JSON, viewport packing, and bug-fix regressions
- Gate: browser package/e2e verification is not covered by this gate
- Gate: `cargo test -p jet-grid-wasm`
- Source: `apps/jet/crates/grid-wasm/src/api.rs`,
  `apps/jet/crates/grid-wasm/src/viewport.rs`
- Evidence: `cargo test -p jet-grid-wasm`; apps/jet/crates/grid-wasm/src/api.rs;
  apps/jet/crates/grid-wasm/src/viewport.rs

### WebGPU Renderer Bridge

Cclab Grid Wasm bridges JavaScript grid views to the native WebGPU renderer
through an async renderer initializer, explicit renderer handle lifecycle,
packed Float32 cell buffers, text-run validation/planning, resize propagation,
and teardown.

- Root WI: none; this capability predates the tracker.
- Surfaces: WASM/JS API: `init_renderer(canvas, width, height, dpr)` and
  `RendererHandle` - Promise-based renderer initialization, packed cell buffer
  rendering, text-run payload validation, resize, and destroy lifecycle
- Gate — behavior: `cargo test -p jet-grid-wasm` - host-portable bridge
  state, packed cell buffer stride validation, text-run payload validation,
  placeholder glyph planning, atlas planning, and resize/render counters
- Gate: browser WebGPU e2e verification is not covered by this gate
- Gate: `cargo test -p jet-grid-wasm`
- Source: `apps/jet/crates/grid-wasm/src/renderer_bridge.rs`,
  `apps/jet/crates/grid-wasm/docs/renderer-bridge-slice-4m.md`
- Evidence: `cargo test -p jet-grid-wasm`;
  apps/jet/crates/grid-wasm/src/renderer_bridge.rs;
  apps/jet/crates/grid-wasm/docs/renderer-bridge-slice-4m.md

### Browser Frame Loop And Resize Bridge

Cclab Grid Wasm provides browser-facing frame-loop and resize state machines
for RAF scheduling, off-screen pause/resume, resize coalescing, DPR clamping,
and typed-array viewport data transfer.

- Root WI: none; this capability predates the tracker.
- Surfaces: WASM/JS API: `WasmView`, `LoopController`, `ResizeDebouncer`,
  `ViewportBuffer` - browser scheduling, visibility, resize debounce, and
  typed-array viewport bridge
- Gate — behavior: `cargo test -p jet-grid-wasm` - RAF loop state
  transitions, visibility pause/resume, resize debounce coalescing/clamping,
  and viewport format packing
- Gate: browser RAF/ResizeObserver integration is not covered by this host gate
- Gate: `cargo test -p jet-grid-wasm`
- Source: `apps/jet/crates/grid-wasm/src/frame_loop.rs`,
  `apps/jet/crates/grid-wasm/src/resize_debouncer.rs`,
  `apps/jet/crates/grid-wasm/src/viewport.rs`
- Evidence: `cargo test -p jet-grid-wasm`;
  apps/jet/crates/grid-wasm/src/frame_loop.rs;
  apps/jet/crates/grid-wasm/src/resize_debouncer.rs;
  apps/jet/crates/grid-wasm/src/viewport.rs
