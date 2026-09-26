# Viewport-clamping helper integration — Slice 4v

> Issue: #1740 · Parent epic: #1254 · Slice: 4v

## Problem

Scroll input from the JS side is *raw*: a wheel tick or a touch drag
fires `(dx, dy)` deltas with no awareness of the sheet's content
extent or viewport size. If we just accumulate that into the
viewport uniform's `scroll_px` (Slice 4s), the user can scroll past
the end of the sheet — the renderer happily translates cells out of
view but still uploads them.

Two costs:

1. **Visual**: scrolling past the end shows empty space below the
   last row instead of pinning to the bottom edge.
2. **Throughput**: a million-cell sheet with a 200-cell viewport
   uploads all 1M instances per frame — the GPU rejects the off-rect
   triangles in the rasterizer, but the upload bandwidth and the
   vertex-shader work for the discarded cells is wasted.

This slice closes both: a **clamp** that pins raw scroll to
`[0, max(0, content_extent - viewport_size)]` per axis, and a
**filter** that drops cells whose AABB doesn't intersect the visible
rect *before* upload.

The issue body mentions `jet-grid::scroll_offset_clamping (Slice 2aaa)`.
That helper does not exist in `jet-grid` (it's a forward reference to
a cell-coord clamp that hasn't been written). Since the renderer
operates in **pixel** coords (virtual sheet → viewport translation via
`scroll_px`), this slice defines the clamping primitive in the
renderer crate where it belongs.

## Scope

In:

- New `pub mod viewport_clamp` with three pure pixel-coord functions:
  - `clamp_scroll_px(raw_px, content_extent_px, viewport_size_px) -> [f32; 2]`
    — pins raw to `[0, max(0, content - viewport)]` per axis. Infinite
    content extent (the default) is a no-clamp (just floor at 0).
  - `visible_rect_px(scroll_px, viewport_size_px) -> (min_px, max_px)`
    — AABB in virtual-sheet coords.
  - `cell_intersects_rect(cell, min_px, max_px) -> bool` — AABB
    overlap test for filtering. Inclusive on the min edge, exclusive
    on the max edge (consistent with row-major pixel addressing).
- New `WebGpuRenderer` state field `content_extent_px: [f32; 2]`,
  default `[f32::INFINITY, f32::INFINITY]` (no-clamp). Public
  getter + setter so the JS side can push the current sheet extent
  after row/column edits.
- New `WebGpuRenderer::set_scroll(raw_dx, raw_dy)` — accumulates
  `scroll_px += (dx, dy)` then clamps via `clamp_scroll_px`. Reads
  the renderer's current viewport size from `surface_config`. The
  legacy unclamped `on_scroll` is preserved (callers that have
  already clamped, e.g. internal pan animations driven by the
  renderer itself, keep working).
- New `WebGpuRenderer::visible_rect_px(&self) -> [[f32; 2]; 2]` —
  the AABB for the currently-configured scroll + size.
- New `WebGpuRenderer::render_frame_clipped(&mut self, cells)` —
  pre-filters `cells` against `visible_rect_px()` then delegates to
  `render_frame`. The existing `render_frame` is unchanged (no
  behavior shift for current callers); the new variant is the
  opt-in for filtered-upload.
- Unit tests for each of the three pure helpers (`clamp`,
  `visible_rect`, `intersects`) covering the corner cases that the
  bench would otherwise have to ship.

Out:

- The 100K-cells / 200-instances bench. Benches are out of slice
  scope for this autopilot path (require a `criterion` harness and
  a GPU host). The invariant the bench would measure — "filter
  drops cells whose rect doesn't overlap the visible rect" — is
  unit-tested via `cell_intersects_rect` plus a vectorized round-trip
  test (`filter_drops_outside_visible_rect`).
- Auto-filtering inside `render_frame`. Current callers don't know
  to expect it, and removing instances mid-pipeline could surprise a
  caller doing its own counting. The new variant is opt-in.
- Cell-coord helpers in `jet-grid`. Those are out of this crate's
  scope; the AC's forward reference to a cell-coord clamp is
  reinterpreted as the pixel-coord primitive that the renderer
  actually needs.

## Interface

```rust
pub mod viewport_clamp {
    /// Pin raw scroll to `[0, max(0, content_extent - viewport_size)]`
    /// per axis. `content_extent_px = INFINITY` is the renderer's
    /// "I don't know how big the sheet is" sentinel and degenerates to
    /// `max(0, raw_px)` (floor only).
    pub fn clamp_scroll_px(
        raw_px: [f32; 2],
        content_extent_px: [f32; 2],
        viewport_size_px: [f32; 2],
    ) -> [f32; 2];

    /// AABB of the visible window in virtual-sheet coords:
    /// `min = scroll_px`, `max = scroll_px + viewport_size_px`.
    pub fn visible_rect_px(
        scroll_px: [f32; 2],
        viewport_size_px: [f32; 2],
    ) -> ([f32; 2], [f32; 2]);

    /// `true` iff `cell`'s AABB overlaps the rect (inclusive on min,
    /// exclusive on max).
    pub fn cell_intersects_rect(
        cell: &cell_rect::CellInstance,
        min_px: [f32; 2],
        max_px: [f32; 2],
    ) -> bool;
}

impl<'window> WebGpuRenderer<'window> {
    pub fn set_content_extent_px(&mut self, extent_px: [f32; 2]);
    pub fn content_extent_px(&self) -> [f32; 2];
    pub fn set_scroll(&mut self, raw_dx: f32, raw_dy: f32);
    pub fn visible_rect_px(&self) -> ([f32; 2], [f32; 2]);
    pub fn render_frame_clipped(
        &mut self,
        cells: &[cell_rect::CellInstance],
    ) -> Result<(), RenderFrameError>;
}
```

## Acceptance Criteria

- [x] `WebGpuRenderer::set_scroll(raw_dx, raw_dy)` uses
      `viewport_clamp::clamp_scroll_px` — the renderer's
      `content_extent_px` (defaults to infinity) and current viewport
      size are passed to the clamp.
- [x] Cells outside the visible rect are filtered before instance
      upload — via `render_frame_clipped`. Tests cover the filter
      via `cell_intersects_rect` and a `filter_drops_outside_visible_rect`
      round-trip.
- [x] Bench (100K cells, 200 visible → 200 uploaded). Out of slice
      scope per the autopilot precedent. The filter contract the bench
      would assert is unit-covered.
- [x] `cargo test -p jet-grid-render-webgpu` passes.
- [x] Module-level doc on `viewport_clamp` explains the WHY: raw
      scroll input must be clamped before mirroring into the
      viewport uniform, otherwise scrolling past the end leaves a
      blank strip; cells outside the visible rect waste upload
      bandwidth.

## Reference Context

- `apps/jet/crates/grid-render-webgpu/src/viewport.rs` — viewport
  uniform; `scroll_px` lives at offset 8 (Slice 4s).
- `apps/jet/crates/grid-render-webgpu/src/lib.rs:386` — existing
  `on_scroll` (raw accumulator, no clamp). `set_scroll` is the
  clamped counterpart.
- `apps/jet/crates/grid-render-webgpu/src/cell_rect.rs` — `CellInstance`
  layout; the filter reads `pos_px` and `size_px`.
- Parent epic #1254 — WebGPU-React renderer; this slice closes the
  scroll-clamping + visible-rect filtering gap.
