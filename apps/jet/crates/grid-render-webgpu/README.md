# Cclab Grid Render Webgpu

## Brief

Cclab Grid Render Webgpu is the native WebGPU rendering runtime for cclab grid
surfaces.

It owns the renderer lifecycle, backend selection, frame/cell-rect pipeline,
viewport/DPR/MSAA state, text glyph pipeline, diagnostics, device-loss recovery,
GPU memory accounting, and headless smoke readback harness. The code has broad
unit coverage, but the configured full `cargo test -p jet-grid-render-webgpu`
gate is currently blocked in this checkout because the local cap memory policy
kills it before integration/doc-test completion.

## Capabilities

A promise with no gate under it is not claimed.

### Capability Index

| Capability | Root WI | Notes |
|---|---:|---|
| WebGPU Renderer Runtime | - | renderer/headless tests start and unit assertions pass until local cap kills the full gate before completion |
| Text Glyph Rendering Pipeline | - | text/glyph units run but the configured full gate is blocked by local memory pressure |
| Diagnostics Recovery And Resource Accounting | - | diagnostics/resource units run but the configured full gate is blocked by local memory pressure |

### WebGPU Renderer Runtime

Cclab Grid Render Webgpu provides the native WebGPU rendering runtime for grid
surfaces, including backend selection, device/queue/surface ownership, frame
orchestration, cell-rect rendering, viewport/DPR/MSAA state, and a headless
smoke-readback harness.

- Root WI: none; this capability predates the tracker.
- Surface: Rust API: `jet_grid_render_webgpu::{WebGpuRenderer, FrameBuilder}`
  - WebGPU renderer lifecycle and surface/frame pipeline.
- Surface: Rust API:
  `jet_grid_render_webgpu::headless::{request_smoke_adapter, HeadlessSmokeRenderer}`
  - surface-less smoke renderer and pixel readback harness.
- Gate — behavior: `cargo test -p jet-grid-render-webgpu` - renderer,
  backend, pipeline, viewport, DPR, instance pool, frame loop, screenshot, and
  headless smoke coverage
- Gate: currently blocked because the local cap memory policy kills the full
  gate before integration/doc-test completion
- Gate: `cargo test -p jet-grid-render-webgpu`
- Gate: `cargo test -p jet-grid-render-webgpu --lib`
- Gate: `cargo test -p jet-grid-render-webgpu --test headless_smoke`
- Source: `apps/jet/crates/grid-render-webgpu/tests/headless_smoke.rs`

| Work Root | Kind | WI | Gate / Evidence |
|---|---|---:|---|
| Renderer runtime smoke contract | epic | - | `cargo test -p jet-grid-render-webgpu`; `cargo test -p jet-grid-render-webgpu --lib`; `cargo test -p jet-grid-render-webgpu --test headless_smoke`; apps/jet/crates/grid-render-webgpu/tests/headless_smoke.rs |
| Headless render readback contract | epic | - | `cargo test -p jet-grid-render-webgpu --test headless_smoke` is killed by local cap memory pressure before completion |

### Text Glyph Rendering Pipeline

Cclab Grid Render Webgpu owns the text rendering data plane for grid cells:
font lookup, font face validation, glyph bitmap rasterization, glyph
cache/atlas descriptors, atlas upload validation, text-pass WGSL/layout, and
glyph shaping value objects.

- Root WI: none; this capability predates the tracker.
- Surfaces: Rust API:
  `jet_grid_render_webgpu::{font_db, font_face, glyph_raster, glyph_cache, glyph_atlas, glyph_atlas_upload, text_pass, shaper}`
  - text/glyph rendering data plane
- Gate — behavior: `cargo test -p jet-grid-render-webgpu` - font
  loading/indexing, font face validation, glyph raster/cache/atlas descriptors
  and upload validation, text pass WGSL/layout, and shaping value objects
- Gate: currently blocked because the local cap memory policy kills the full
  gate before integration/doc-test completion
- Gate: `cargo test -p jet-grid-render-webgpu`
- Source: `apps/jet/crates/grid-render-webgpu/src/font_db.rs`,
  `apps/jet/crates/grid-render-webgpu/src/text_pass.rs`,
  `apps/jet/crates/grid-render-webgpu/docs`
- Evidence: `cargo test -p jet-grid-render-webgpu`;
  apps/jet/crates/grid-render-webgpu/src/font_db.rs;
  apps/jet/crates/grid-render-webgpu/src/text_pass.rs;
  apps/jet/crates/grid-render-webgpu/docs

### Diagnostics Recovery And Resource Accounting

Cclab Grid Render Webgpu exposes renderer diagnostics and recovery helpers for
wgpu validation signal, tracing spans, device-loss observation/recovery, GPU
memory estimation, frame timing, viewport clamping, and screenshot/readback
invariants.

- Root WI: none; this capability predates the tracker.
- Surfaces: Rust API:
  `jet_grid_render_webgpu::{validation, tracing_setup, gpu_memory, frame_timing, lost_context, viewport_clamp}`
  - diagnostics, recovery, and resource-accounting helpers
- Gate — behavior: `cargo test -p jet-grid-render-webgpu` - validation
  flags/log bridge, tracing setup, device-loss status/recovery errors, GPU
  memory math, frame timing, viewport clamp, and screenshot/readback invariants
- Gate: currently blocked because the local cap memory policy kills the full
  gate before integration/doc-test completion
- Gate: `cargo test -p jet-grid-render-webgpu`
- Source: `apps/jet/crates/grid-render-webgpu/src/validation.rs`,
  `apps/jet/crates/grid-render-webgpu/src/gpu_memory.rs`,
  `apps/jet/crates/grid-render-webgpu/src/lost_context.rs`
- Evidence: `cargo test -p jet-grid-render-webgpu`;
  apps/jet/crates/grid-render-webgpu/src/validation.rs;
  apps/jet/crates/grid-render-webgpu/src/gpu_memory.rs;
  apps/jet/crates/grid-render-webgpu/src/lost_context.rs
