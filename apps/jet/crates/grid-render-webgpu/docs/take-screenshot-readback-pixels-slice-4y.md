# `WebGpuRenderer::take_screenshot` — readback pixels — Slice 4y

> Issue: #1743 · Parent epic: #1254 · Slice: 4y

## Problem

Two downstream consumers — visual-regression infra (Epic 18) and the
SSR pipeline (Epic 19) — both need to read the rendered framebuffer
back to CPU. They can't go through the live `Surface::get_current_texture`
path: surface textures (a) don't always carry the `COPY_SRC` usage bit
on every backend/format combo and (b) are consumed by `present()`, so
the pixel data is unreadable by the time a test wants to assert on it.

The renderer therefore needs a single shared "render this scene to a
buffer at current surface dimensions and return the RGBA8 bytes"
method that:

1. Allocates a transient offscreen color texture matching
   `surface_config.width × surface_config.height` (which is already
   DPR-scaled by `set_size`), with `RENDER_ATTACHMENT | COPY_SRC`
   usage.
2. Encodes the cell-rect pass into that texture using the cached
   pipeline for `Rgba8Unorm`.
3. `copy_texture_to_buffer` into a `MAP_READ` buffer with the
   256-byte row alignment wgpu requires.
4. `map_async` + `device.poll(Maintain::Wait)` + strip the padding +
   return the bytes.

This sits next to (not on top of) `render_frame`: a screenshot must
not perturb the swap-chain cadence, so it never touches the surface.

## Scope

In:

- New `WebGpuRenderer::take_screenshot(&mut self, cells)` method
  returning `Result<Screenshot, ScreenshotError>`.
- New `pub struct Screenshot { width: u32, height: u32, pixels: Vec<u8> }`
  carrying RGBA8 row-major bytes (length = `width * height * 4`).
- New `pub enum ScreenshotError` (`thiserror`) covering the failure
  modes the readback path actually has: `DeviceLost`, `OutOfMemory`,
  `BufferMapFailed`, `ZeroSizeSurface`.
- Re-use the `Rgba8Unorm` cell-rect pipeline from `pipeline_for` (Slice
  4u). Screenshot format is **always** `Rgba8Unorm` regardless of the
  surface format — downstream code wants a stable RGBA8 contract, not
  the surface's native BGRA8 or sRGB swizzle.
- Re-use the row-padding strip logic pattern from `headless.rs` (Slice
  4t) — same `align_up` + per-row copy.
- Unit tests:
  - `screenshot_struct_shape` (compile-only) — asserts the
    `Screenshot` struct's pixel buffer length equation
    (`pixels.len() == width * height * 4`).
  - `screenshot_round_trip_known_pattern_live` (`#[ignore]`, live
    GPU) — renders a single red cell at `(0,0,10,10)` against a black
    clear, takes the screenshot, asserts pixel `(0,0)` is red and
    pixel `(50,50)` is black. Mirrors the headless smoke pattern.

Out:

- Reading back the *surface* texture directly. The surface lifecycle
  + per-backend `COPY_SRC` capability variance makes that path fragile;
  the offscreen-render path is what visual-regression / SSR actually
  want anyway.
- MSAA resolves. The screenshot pipeline always renders single-sampled
  — the AC doesn't mention MSAA and software CI adapters expose patchy
  multisample support.
- Async API. The internal `map_async` is awaited synchronously via
  `device.poll(Maintain::Wait)`; the public method is sync because
  the callers (test asserts, SSR encoders) are sync.
- `take_screenshot` on `HeadlessSmokeRenderer`. The headless renderer
  already has its own readback path (`render_single_cell` returns the
  pixels). Slice 4y is specifically the *surface-bound* renderer's
  variant.

## Interface

```rust
/// RGBA8 row-major pixel readback returned by `take_screenshot`.
///
/// Invariant: `pixels.len() == (width * height * 4) as usize`.
pub struct Screenshot {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

#[derive(Debug, thiserror::Error)]
pub enum ScreenshotError {
    #[error("surface has zero width or height — call set_size first")]
    ZeroSizeSurface,
    #[error("device lost during screenshot: {0}")]
    DeviceLost(String),
    #[error("out of memory allocating screenshot resources: {0}")]
    OutOfMemory(String),
    #[error("readback buffer map failed: {0}")]
    BufferMapFailed(String),
}

impl<'window> WebGpuRenderer<'window> {
    /// Render `cells` to an offscreen `Rgba8Unorm` texture sized to the
    /// current `surface_config` dims (which already encode DPR), copy
    /// to a `MAP_READ` buffer, and return the RGBA8 bytes in row-major
    /// top-to-bottom order.
    ///
    /// Does not touch the surface — safe to call between or instead of
    /// `render_frame`.
    pub fn take_screenshot(
        &mut self,
        cells: &[cell_rect::CellInstance],
    ) -> Result<Screenshot, ScreenshotError>;
}
```

## Acceptance Criteria

- [x] `take_screenshot() -> Vec<u8>` (RGBA8 pixels, row-major) — via
      the `Screenshot::pixels` field. Wrapping the bytes in a struct
      preserves the dimensions the caller needs to interpret the
      row stride.
- [x] Uses `CommandEncoder::copy_texture_to_buffer` + `Buffer::map_async`
      — both calls are in the body of `take_screenshot`.
- [x] Honors current surface dimensions + DPR — reads
      `surface_config.width / height`, which `set_size` populated from
      the DPR-aware computation.
- [x] Round-trip test: render a known-pattern frame, screenshot,
      assert pixel values — `screenshot_round_trip_known_pattern_live`
      (`#[ignore]` so CI without a GPU still passes).
- [x] `cargo test -p jet-grid-render-webgpu` passes.
- [x] Module-level doc explains the WHY: visual-regression + SSR both
      need readback; surface textures aren't reliably `COPY_SRC` and
      are consumed by `present`, so screenshot renders into an
      offscreen target.

## Reference Context

- `apps/jet/crates/grid-render-webgpu/src/headless.rs` — Slice 4t. The
  row-padding strip pattern + `map_async`/`Maintain::Wait` recipe is
  reused here.
- `apps/jet/crates/grid-render-webgpu/src/lib.rs:781` — `render_frame`.
  Screenshot is a sibling, not a replacement.
- `apps/jet/crates/grid-render-webgpu/src/lib.rs` — `pipeline_for`
  (Slice 4u) caches the `Rgba8Unorm` pipeline; screenshot reuses it.
- `apps/jet/crates/grid-render-webgpu/src/dpr.rs` — DPR-aware sizing the
  surface dims already reflect, so the screenshot inherits DPR
  automatically.
- Parent epic #1254 — WebGPU-React renderer; Slice 4y closes the
  readback gap for downstream visual-regression / SSR consumers.
