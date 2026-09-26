# RenderPass orchestration — cell pass per frame — Slice 4e

> **Issue**: #1723
> **Parent epic**: #1254 (WebGPU-React renderer)
> **Depends on**: #1719 (Slice 4a — WebGpuRenderer wrapper),
>                 #1720 (Slice 4b — cell-rect pipeline),
>                 #1721 (Slice 4c — viewport uniform),
>                 #1722 (Slice 4d — instance buffer pool)
> **Status**: in-flight

## Problem

Slices 4a–4d built the GPU resources every frame needs (device/queue,
cell-rect pipeline, viewport uniform, instance buffer pool) but no slice
yet *issues a draw*. This slice closes that loop: one method on
`WebGpuRenderer` that, given a slice of `CellInstance`, performs the
entire per-frame sequence — acquire the swap-chain texture, encode a
single render pass with a configurable clear color, bind the cell-rect
pipeline + viewport bind-group + instance buffer, emit
`draw(0..4, 0..N)` for an instanced triangle strip, and submit + present.

Why now: every later slice (multi-pass orchestration, text pass, GPU
profiler) needs this as the spine to plug into. Keeping the cell pass
isolated here — not yet a generic pass scheduler — keeps the spine
simple and the tests grounded.

## Scope

In:

- New method `WebGpuRenderer::render_frame(&mut self, cells: &[CellInstance]) -> Result<(), RenderFrameError>`.
- New configurable `clear_color: wgpu::Color` field on `WebGpuRenderer`,
  default `wgpu::Color::BLACK`, with a `set_clear_color` setter.
- New `instance_pool: InstanceBufferPool` field on `WebGpuRenderer` (wires
  in the Slice 4d utility). Frame slot is `0` for this slice — multi-frame
  pacing is a future slice.
- New `RenderFrameError` enum: `SurfaceLost`, `Outdated`, `Timeout`,
  `OutOfMemory`, `Other(String)`. These map 1:1 to
  `wgpu::SurfaceError` variants so callers can treat surface loss as a
  reconfigure-then-retry event (the standard wgpu idiom).
- The render pass attaches the surface texture's view as the single color
  target, clears to `clear_color`, and discards depth (no depth attachment).

Out:

- Multiple render passes per frame (text pass, overlays — later slices).
- Multi-frame-in-flight / fences — instance pool slot is hard-coded to 0.
- Triple-buffered command encoders.
- HDR / wide-gamut handling beyond the surface format the existing
  Slice 4a config already picks.
- Returning the encoded `CommandBuffer` for caller composition — this
  slice owns submit + present; we'll generalise when a second pass needs
  to share an encoder.
- Empty-cells handling beyond the obvious: `cells.len() == 0` still runs
  the clear (so the user sees the background) and skips the draw call.

## Interface

```rust
// apps/jet/crates/grid-render-webgpu/src/lib.rs

impl<'window> WebGpuRenderer<'window> {
    /// Set the background clear color used by `render_frame`. Idempotent;
    /// effective on the next frame. Defaults to BLACK on construction.
    ///
    /// @issue #1723
    pub fn set_clear_color(&mut self, color: wgpu::Color);

    /// Borrow the configured clear color. Primarily for tests.
    ///
    /// @issue #1723
    pub fn clear_color(&self) -> wgpu::Color;

    /// Render one frame's worth of cells:
    ///
    /// 1. Acquire the current `SurfaceTexture`.
    /// 2. Build a single `CommandEncoder` and one color render pass that
    ///    clears to `self.clear_color`.
    /// 3. Upload `cells` into the slot-0 instance buffer (or grow the
    ///    pool slot if needed).
    /// 4. Build (or reuse the cached) cell-rect pipeline for the current
    ///    surface format.
    /// 5. Bind pipeline, the persistent viewport bind-group, and the
    ///    instance buffer.
    /// 6. Emit `draw(0..4, 0..cells.len())`. Skipped iff `cells` is empty.
    /// 7. Submit and present.
    ///
    /// If `cells` is empty the pass still clears (so the surface gets a
    /// fresh frame) but no draw is issued; the instance pool is left
    /// untouched.
    ///
    /// @issue #1723
    pub fn render_frame(&mut self, cells: &[CellInstance]) -> Result<(), RenderFrameError>;
}

/// Errors surfaced from [`WebGpuRenderer::render_frame`].
///
/// @issue #1723
#[derive(Debug, thiserror::Error)]
pub enum RenderFrameError {
    /// Surface is gone (window closed / GPU reset). Caller should drop
    /// the renderer.
    #[error("surface lost")]
    SurfaceLost,
    /// Swap chain is stale relative to the current surface configuration;
    /// caller should call `on_resize(self.size())` and retry next frame.
    #[error("surface configuration out of date — caller should reconfigure")]
    Outdated,
    /// `get_current_texture` timed out — usually transient.
    #[error("surface acquire timed out")]
    Timeout,
    /// GPU device is out of memory.
    #[error("out of memory")]
    OutOfMemory,
    /// Anything else wgpu surfaces.
    #[error("render frame error: {0}")]
    Other(String),
}
```

## Acceptance criteria

- [x] `render_frame(cells: &[CellInstance]) -> Result<(), RenderFrameError>`.
- [x] Acquires `SurfaceTexture`, creates view + `CommandEncoder`.
- [x] Begins one render pass with a configurable clear color (`set_clear_color`).
- [x] Binds pipeline + bind group + instance buffer.
- [x] Emits `draw(0..4, 0..cells.len() as u32)` — one instanced strip
      (skipped on empty cells, which still get the clear).
- [x] `queue.submit` + `frame.present`.
- [x] `cargo test` passes for `jet-grid-render-webgpu` (live-GPU path
      gated behind `#[ignore]`).
- [x] Module-level docs explain WHY (frame-loop ownership, error
      classification, empty-cells policy).

## Reference context

- `apps/jet/crates/grid-render-webgpu/src/lib.rs` — Slice 4a `WebGpuRenderer`
  to extend.
- `apps/jet/crates/grid-render-webgpu/src/cell_rect.rs` — `CellInstance` byte
  layout, pipeline config.
- `apps/jet/crates/grid-render-webgpu/src/instance_pool.rs` — Slice 4d
  `InstanceBufferPool::get_or_grow` for instance-buffer reuse.
- `apps/jet/crates/grid-render-webgpu/src/viewport.rs` — viewport bind group
  already on the renderer.
- wgpu 24 docs: `SurfaceError::{Timeout, Outdated, Lost, OutOfMemory,
  Other}` covers the acquire failure space. Map 1:1 in `RenderFrameError`.
