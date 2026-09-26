# Scroll-driven uniform update — Slice 4s

> Issue: #1737 · Parent epic: #1254 · Slice: 4s

## Problem

Cells live in virtual-sheet coordinates (the entire scrollable sheet,
not the visible window). When the user scrolls, the cell *contents*
don't change — only the visible window into the sheet does. The right
GPU-side primitive is therefore a *translation in the vertex shader*:
the shader subtracts the current scroll offset from each cell's
virtual position to produce a physical-pixel position for the visible
window. The CPU side updates exactly one uniform — never the per-cell
instance buffer.

Today's renderer has the bones: `ViewportUniforms` is a 16-byte struct
with `size_px: vec2<f32>` at offset 0 and an 8-byte `_pad` filler that
keeps the WGSL 16-byte alignment requirement satisfied. The padding
slot is unused. This slice claims it for `scroll_px: vec2<f32>` and
wires the renderer + WGSL to translate by it.

The performance contract is "one `Queue::write_buffer` per scroll event,
no instance re-upload" — and concretely, that write should be the
*minimum-possible* payload: 8 bytes at offset 8 in the viewport
uniform buffer, not a full 16-byte overwrite.

## Scope

In:

- Replace `ViewportUniforms._pad: [f32; 2]` with
  `ViewportUniforms.scroll_px: [f32; 2]`. The 16-byte total size
  (WGSL alignment invariant) is preserved.
- Add `ViewportUniforms::with_scroll(w, h, scroll: [f32; 2])`
  constructor. Keep `new(w, h)` (scroll = `[0.0, 0.0]`) for
  callers that don't care about scroll.
- Update WGSL `Viewport` struct in `CELL_RECT_WGSL` to add
  `scroll_px: vec2<f32>` and subtract it in `vs_main` so cells in
  virtual-sheet coords appear at scroll-translated physical positions.
- New field `scroll_px: [f32; 2]` on `WebGpuRenderer` tracking the
  accumulated scroll offset.
- New `WebGpuRenderer::on_scroll(dx_px, dy_px)`: accumulates the
  delta into `self.scroll_px`, then writes ONLY the 8 scroll bytes to
  the uniform buffer at offset 8 via `Queue::write_buffer`. Does not
  touch instance buffers, does not rebuild the bind group, does not
  re-encode any pass. Cheap.
- New `WebGpuRenderer::scroll_px() -> [f32; 2]` getter.
- New `WebGpuRenderer::reset_scroll()` — sets to `[0.0, 0.0]` and
  writes the partial uniform. Useful for tests + "scroll to top"
  UX hooks.
- Internal call sites that currently invoke `set_viewport(
  ViewportUniforms::new(w, h))` (constructor, `on_resize`,
  `on_resize_logical`, `set_dpr`, `try_recover`) now pass the
  renderer's tracked scroll through `with_scroll` so a resize
  preserves the user's scroll position.

Out:

- Smooth/momentum scrolling, scroll inertia, scroll-bounds clamping.
  This slice exposes a raw delta accumulator; UX layers can build the
  smoothing they need on top.
- Scroll-driven culling. The full cell set is still uploaded; culling
  off-screen cells is a separate optimisation (later slice in the
  epic).
- Bench acceptance ("< 5% CPU @ 60fps for 5s on a 1080p mid-tier
  laptop"). Requires a bench harness + GPU host; explicitly a
  measurement follow-up. The implementation invariant this slice
  enforces — "single 8-byte uniform write per scroll event, zero
  instance work" — is what the bench would measure.

## Interface

```rust
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct ViewportUniforms {
    pub size_px: [f32; 2],
    pub scroll_px: [f32; 2],
}

impl ViewportUniforms {
    pub fn new(width_px: f32, height_px: f32) -> Self; // scroll = [0,0]
    pub fn with_scroll(width_px: f32, height_px: f32, scroll_px: [f32; 2]) -> Self;
}

impl<'window> WebGpuRenderer<'window> {
    /// Accumulate a scroll delta in physical pixels. Writes the new
    /// scroll vector to the uniform buffer in a single 8-byte
    /// `Queue::write_buffer` at offset 8. Does not re-upload instances
    /// or rebuild any pipeline state.
    pub fn on_scroll(&mut self, dx_px: f32, dy_px: f32);

    /// Current accumulated scroll offset in physical pixels.
    pub fn scroll_px(&self) -> [f32; 2];

    /// Reset scroll to `[0.0, 0.0]` and push the change to the GPU.
    pub fn reset_scroll(&mut self);
}
```

WGSL:

```wgsl
struct Viewport {
    size_px:   vec2<f32>,
    scroll_px: vec2<f32>,
};

// vs_main translates by scroll_px before NDC projection:
let px = inst.pos_px + corner * inst.size_px - viewport.scroll_px;
```

Coordinate convention: `scroll_px` represents how far the visible
window has moved DOWN-RIGHT into the virtual sheet. So `on_scroll(0,
+100)` shifts the window 100px down → visible content appears to move
UP. Matches conventional UI scroll semantics.

## Acceptance Criteria

- [x] `WebGpuRenderer::on_scroll(dx, dy)` updates only the uniform —
      no instance buffer touch, no pipeline rebuild.
- [x] Single `Queue::write_buffer` per scroll event, 8 bytes at the
      `scroll_px` offset (not a full 16-byte write).
- [x] Visual: WGSL `vs_main` subtracts `scroll_px` so cell positions
      update without flicker. (Verified by shader-source diff + naga
      parse test.)
- [x] Internal `set_viewport` callers preserve scroll across resize
      / DPR change / device recovery.
- [x] `cargo test -p jet-grid-render-webgpu` passes.
- [x] Module-level docs explain the WHY: virtual-sheet coords mean
      scroll is a uniform translation, not an instance re-upload.

## Reference Context

- `apps/jet/crates/grid-render-webgpu/src/viewport.rs:27-45` — current
  `ViewportUniforms`; this slice converts `_pad` into `scroll_px`.
- `apps/jet/crates/grid-render-webgpu/src/cell_rect.rs:27-64` —
  `CELL_RECT_WGSL`; this slice extends `Viewport` and subtracts
  `scroll_px` in `vs_main`.
- `apps/jet/crates/grid-render-webgpu/src/lib.rs:345-354` — current
  `set_viewport` writes the full 16 bytes; the new `on_scroll` writes
  just the 8 scroll bytes at offset 8.
- Sibling Slice 4c (#1721) introduced the persistent uniform buffer
  + bind group; this slice rides on that infrastructure.
