# Clear-color configuration on WebGpuRenderer — Slice 4o

> Issue: #1733 · Parent epic: #1254 · Slice: 4o

## Problem

The render pass clears the color attachment before drawing every frame.
That clear color must match the host app's theme (light / dark mode) so
the first paint does not flash a contrasting color before the first
real frame's draws land. Slice 4e (#1723) wired the field through to
the render pass, but two AC bullets on the parent epic (#1254) still
need work:

1. The default is `wgpu::Color::BLACK`. The parent epic specifies an
   **opaque white** default — most embedding apps render light-mode UI
   on first paint, so white is the lower-flash default and the
   one-line override (`set_clear_color([0.0, 0.0, 0.0, 1.0])`) is the
   uncommon dark-mode path.
2. The setter takes `wgpu::Color` (four `f64`s). The epic API contract
   specifies `[f32; 4]` — embedding apps already hold theme colors as
   `f32` RGBA arrays (the same shape `CellInstance::color` uses), and
   the `wgpu::Color` shape leaks the renderer's GPU choice into the
   embedding app.

## Scope

In:

- Flip the default from `wgpu::Color::BLACK` to opaque white
  (`[1.0, 1.0, 1.0, 1.0]`).
- Re-shape `WebGpuRenderer::set_clear_color` to take `[f32; 4]` and
  `WebGpuRenderer::clear_color` to return `[f32; 4]`. Internal storage
  stays `wgpu::Color` — the `[f32; 4]` ↔ `wgpu::Color` conversion is a
  private helper.
- Update the existing live-GPU test (`render_frame_runs_end_to_end_live`)
  to drive the new signature and to additionally exercise a black
  clear over an empty cell list (the AC's "black surface" path).
- Add a non-GPU unit test that pins the new default and the
  set/get round-trip.

Out:

- Reading the surface back to assert pixel-level "the cleared surface
  is actually black". That requires a `COPY_SRC` texture + buffer
  map-async dance and is not what this slice is for. The existing
  `#[ignore]`'d live-GPU test runs `render_frame(&[])` against the
  configured clear color — if wgpu's clear-pass is buggy this is the
  layer that surfaces it, not this slice's tests.
- Per-render-pass clear-color override. The field-on-renderer model is
  intentional (configure once, hot path stays argument-light); slice
  4e (#1723) documents the WHY.
- Touching `RenderFrameError`, `try_acquire_frame`, or any other
  Slice-4x machinery — this slice is contained to the public
  setter/getter signature + default value.

## Interface

```rust
impl<'window> WebGpuRenderer<'window> {
    /// Override the background clear color used by [`render_frame`].
    /// The new value takes effect on the next frame; in-flight encoders
    /// are not affected.
    ///
    /// Components are linear-space `[r, g, b, a]` in `[0.0, 1.0]`,
    /// matching the shape `CellInstance::color` uses so embedding apps
    /// hold one type of theme color throughout.
    pub fn set_clear_color(&mut self, rgba: [f32; 4]);

    /// Borrow the currently configured clear color as `[r, g, b, a]`.
    /// Defaults to opaque white (`[1.0, 1.0, 1.0, 1.0]`) until
    /// overridden by [`set_clear_color`].
    pub fn clear_color(&self) -> [f32; 4];
}
```

Default at construction is `[1.0, 1.0, 1.0, 1.0]`.

## Acceptance Criteria

- [x] `WebGpuRenderer::set_clear_color([f32; 4])` exists.
- [x] Default = opaque white (`[1.0, 1.0, 1.0, 1.0]`).
- [x] Stored on the renderer, read in `render_frame`.
- [x] Test: setting black and calling `render_frame(&[])` against a
      1×1 headless surface returns `Ok(())` (live-GPU, `#[ignore]`d).
- [x] Non-GPU unit test pins the white default and the round-trip
      through `set_clear_color` / `clear_color`.
- [x] `cargo test -p jet-grid-render-webgpu` passes.

## Reference Context

- `apps/jet/crates/grid-render-webgpu/src/lib.rs:83-86` — `clear_color`
  field on `WebGpuRenderer`.
- `apps/jet/crates/grid-render-webgpu/src/lib.rs:245` — current default
  (BLACK; this slice flips to white).
- `apps/jet/crates/grid-render-webgpu/src/lib.rs:409-426` — current
  `set_clear_color` / `clear_color` signatures (this slice re-shapes
  to `[f32; 4]`).
- `apps/jet/crates/grid-render-webgpu/src/lib.rs:450-458` — `render_frame`
  reads the field for the load-op clear value (unchanged here).
- `apps/jet/crates/grid-render-webgpu/docs/render-pass-orchestration-slice-4e.md` —
  parent slice that introduced the field + plumbed it into the pass.
