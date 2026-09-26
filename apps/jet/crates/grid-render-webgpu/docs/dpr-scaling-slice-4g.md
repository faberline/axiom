# Device-pixel-ratio (DPR) scaling — Slice 4g

> **Issue**: #1725
> **Parent epic**: #1254 (WebGPU-React renderer)
> **Depends on**: #1719 (Slice 4a — renderer wrapper),
> #1721 (Slice 4c — viewport uniform).
> **Status**: in-flight

## Problem

Browsers expose two coordinate systems: a *logical* / CSS-pixel space
that React knows about (where a `100×100` div is genuinely 100 CSS
pixels), and a *physical* / device-pixel space that the GPU draws into
(where a Retina screen at `dpr = 2` makes that same div 200×200
actual pixels). The renderer must:

1. Configure its `wgpu::Surface` at *physical* pixels (the texture the
   GPU clears and draws to has the device's true resolution — crisp
   text and edges on Retina / high-DPI displays).
2. Push *physical* pixel dimensions into the viewport uniform (so the
   WGSL pos → NDC math projects correctly without each shader having
   to know about the DPR).
3. Let CSS handle the downscale: the canvas element's *CSS* size is
   the logical size, and the browser scales the physical-pixel texture
   to fit that box.
4. Provide a conversion path so future hit-test code (still in the
   React side at this slice — no hit-test tree exists in this crate
   yet) can divide a physical pointer coordinate by DPR to land back
   in the element bbox tree's logical space.

Today the renderer treats whatever size it's handed as physical pixels.
That's correct *if* the React layer multiplies by DPR before calling
`on_resize`, but it's an undocumented contract — and the React layer
also needs the inverse (physical → logical) for pointer events. Slice
4g formalizes both directions in the renderer.

## Scope

In:

- New `WebGpuRenderer::dpr() -> f32` and `WebGpuRenderer::set_dpr(f32)`.
  `set_dpr` re-configures the surface with the new physical size
  `(logical * dpr)` and re-seeds the viewport uniform.
- New `WebGpuRenderer::logical_size() -> (u32, u32)` companion to the
  existing physical-size getter (`size`). The renderer tracks both.
- New `WebGpuRenderer::on_resize_logical(logical: (u32, u32))` —
  multiplies by current DPR, configures the surface, pushes physical
  dims into the viewport uniform. This is the entry point React
  resize handlers should use.
- Existing `WebGpuRenderer::on_resize(physical: (u32, u32))` stays as
  the physical-pixel entry; it's retained because some host bindings
  (raw canvas resize observers) report physical px directly.
- New module-level pure helpers (testable without a GPU):
  - `compute_physical_size(logical: (u32, u32), dpr: f32) -> (u32, u32)`.
  - `logical_to_physical_f32(logical: (f32, f32), dpr: f32) -> (f32, f32)`.
  - `physical_to_logical_f32(physical: (f32, f32), dpr: f32) -> (f32, f32)`.
- Renderer instance methods that delegate to the helpers using its
  current `dpr`: `to_physical((f32, f32))`, `to_logical((f32, f32))`.

Out:

- Hit-test tree integration. This crate exposes the conversion math;
  consumers (a future jet-grid-wasm / element bbox slice) wire it
  into actual pointer dispatch.
- DPR change detection. The React layer is expected to call
  `set_dpr` itself (driven by a `window.matchMedia('(resolution: ...)')`
  listener). No `matchMedia` polling here.
- Per-axis DPR. Browsers expose a scalar; if a future platform
  introduces non-uniform DPR we'll revisit.

## Interface

```rust
// apps/jet/crates/grid-render-webgpu/src/dpr.rs (pure helpers — testable
// without a GPU)

/// Multiply a logical size by `dpr`, round half-away-from-zero, and
/// clamp each axis to ≥ 1 (wgpu rejects zero-sized surface configs).
pub fn compute_physical_size(logical: (u32, u32), dpr: f32) -> (u32, u32);

/// Multiply f32 logical coords by `dpr`.
pub fn logical_to_physical_f32(logical: (f32, f32), dpr: f32) -> (f32, f32);

/// Divide f32 physical coords by `dpr`. `dpr <= 0` is treated as 1.0
/// (avoids div-by-zero / NaN from misconfigured callers).
pub fn physical_to_logical_f32(physical: (f32, f32), dpr: f32) -> (f32, f32);

impl<'window> WebGpuRenderer<'window> {
    pub fn dpr(&self) -> f32;
    pub fn set_dpr(&mut self, dpr: f32);
    pub fn logical_size(&self) -> (u32, u32);
    pub fn on_resize_logical(&mut self, logical: (u32, u32));
    pub fn to_physical(&self, logical: (f32, f32)) -> (f32, f32);
    pub fn to_logical(&self, physical: (f32, f32)) -> (f32, f32);
}
```

## Acceptance criteria

- [x] `WebGpuRenderer` tracks current DPR (new `dpr: f32` field, default
      `1.0`; `dpr()` / `set_dpr` accessors).
- [x] Surface configured at `logical_size * dpr` — `on_resize_logical`
      multiplies via `compute_physical_size` and calls
      `surface.configure` with the physical dims.
- [x] Viewport uniforms use physical-pixel dimensions — the existing
      `set_viewport` call inside `on_resize` / `on_resize_logical`
      already feeds `surface_config.width / height` (physical), which
      is what we want; doc the invariant and lock it down with a
      compile-time test on the public API.
- [x] Hit-test math divides by DPR before reaching the element bbox
      tree — exposed via `to_logical((physical_x, physical_y))` and
      the pure `physical_to_logical_f32` helper.
- [x] Test: `compute_physical_size((100, 100), 2.0) == (200, 200)` —
      unit test exercises the round/clamp + dpr=2 path.
- [x] `cargo test -p jet-grid-render-webgpu` passes.
- [x] Module-level docs explain WHY (CSS-px vs device-px invariant +
      `dpr <= 0` defensive fallback).

## Reference context

- `apps/jet/crates/grid-render-webgpu/src/lib.rs` — `WebGpuRenderer`
  (where DPR fields land), `on_resize` (existing physical-px entry).
- `apps/jet/crates/grid-render-webgpu/src/viewport.rs` —
  `ViewportUniforms::new(width_px, height_px)` already takes physical
  px; no change needed.
- WebGPU spec: `GPUSurfaceConfiguration::size` is in physical / device
  pixels. CSS controls the canvas's logical size.
