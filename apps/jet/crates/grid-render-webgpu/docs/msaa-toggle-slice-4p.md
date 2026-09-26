# MSAA toggle (off by default, on for native) — Slice 4p

> Issue: #1734 · Parent epic: #1254 · Slice: 4p

## Problem

The cell-rect pass currently renders single-sampled (count=1). Axis-
aligned cell rects don't *need* MSAA on web — quad edges land on pixel
boundaries and the cost of resolves on integrated GPUs is non-trivial —
but native targets (Metal / Vulkan / DX12) generally have cheap 4× MSAA
on modern desktop hardware, and a 4× pass cleans up rotated overlays
and future stroke passes the parent epic already plans for.

The renderer needs:

- A way to flip MSAA on and off at runtime (`set_msaa_count`),
- The pipeline cache to invalidate on flip (`MultisampleState.count`
  is baked into the pipeline descriptor),
- A resolve target — an intermediate multisampled texture matching
  surface size and format — to exist only while count > 1, and
- A target-aware default: count=1 on web, count=4 on native.

## Scope

In:

- New field `msaa_count: u32` on `WebGpuRenderer`, defaulting to 1 on
  `wasm32` and 4 elsewhere (via a `cfg`-gated `const`).
- New field `msaa_view: Option<wgpu::TextureView>` (Some iff count > 1)
  plus the backing `wgpu::Texture` so it isn't dropped while in use.
- `WebGpuRenderer::set_msaa_count(count: u32)` — accepts `1` or `4`,
  ignores other values (out of scope for this slice: 2× / 8× — they'd
  need adapter feature negotiation which is Slice 4q).
- `WebGpuRenderer::msaa_count() -> u32` getter.
- Pipeline cache re-keyed from `HashMap<TextureFormat, _>` to
  `HashMap<(TextureFormat, u32), _>` — `create_cell_pipeline` now
  takes the count as a parameter; `set_msaa_count` clears the cache.
- MSAA texture (re)allocated in three places: constructor, `on_resize`
  / `on_resize_logical` / `set_dpr` (any path that reconfigures the
  surface dimensions), and `set_msaa_count` (when transitioning from
  count=1 → count=4, or when the surface format has changed since the
  last allocation).
- `FrameBuilder::encode_cell_pass` reads the renderer's MSAA state and
  selects the color-attachment topology:
  - count == 1: `view = surface_view, resolve_target = None` (today's
    behavior).
  - count > 1: `view = msaa_view, resolve_target = Some(surface_view),
    StoreOp::Store` — wgpu resolves on present.

Out:

- Adapter feature negotiation for arbitrary sample counts. wgpu
  guarantees `count: 1` and `count: 4` for color-renderable formats
  without any feature flag; 2× / 8× / 16× require feature checks and
  surface format compatibility — that's Slice 4q (#1735).
- Per-pipeline MSAA override. The renderer is the sole owner of
  MSAA state for now — all passes use the same count. A future text
  pass slice can revisit this if it wants single-sampled text over
  4× cells.
- Depth/stencil. The cell pass is colorless-only; if a later slice
  adds depth, it must reconcile its own multisample state.

## Interface

```rust
impl<'window> WebGpuRenderer<'window> {
    /// Set the MSAA sample count used by the cell-rect pass. Accepts
    /// `1` (single-sampled, web default) or `4` (4× MSAA, native
    /// default). Other values are ignored — see Slice 4q (#1735) for
    /// adapter-feature-gated counts.
    ///
    /// Side effects: clears the pipeline cache (the next
    /// `create_cell_pipeline` call rebuilds against the new count)
    /// and (re)allocates the MSAA texture when the new count is > 1.
    pub fn set_msaa_count(&mut self, count: u32);

    /// Current MSAA sample count. Defaults to `1` on `wasm32`, `4` on
    /// native.
    pub fn msaa_count(&self) -> u32;
}
```

## Acceptance Criteria

- [x] `WebGpuRenderer::set_msaa_count(1 | 4)` exists; other values
      are silently ignored.
- [x] Pipeline rebuild on toggle — cache cleared by `set_msaa_count`.
- [x] Resolve target allocated when count > 1 (Some `msaa_view`),
      dropped when count == 1 (None).
- [x] Default: 1 on web (`cfg(target_arch = "wasm32")`), 4 on native.
- [x] `on_resize` (and friends) reallocate the MSAA texture so the
      resolve target stays in sync with surface dimensions.
- [x] Non-GPU unit tests pin the target-conditional default and the
      `set_msaa_count` value-validation behavior.
- [x] `cargo test -p jet-grid-render-webgpu` passes.

## Reference Context

- `apps/jet/crates/grid-render-webgpu/src/lib.rs:365-407` —
  `create_cell_pipeline`; this slice re-keys the cache.
- `apps/jet/crates/grid-render-webgpu/src/lib.rs:399` —
  `multisample: wgpu::MultisampleState::default()`; this slice threads
  the count through.
- `apps/jet/crates/grid-render-webgpu/src/frame.rs:122-145` —
  `encode_cell_pass` render-pass descriptor; this slice adjusts the
  color attachment based on MSAA state.
- `apps/jet/crates/grid-render-webgpu/src/lib.rs:266-275` — `on_resize`,
  one of the surface-reconfigure paths that must reallocate the MSAA
  texture.
- Sibling slice in progress: `adapter-feature-negotiation-slice-4q.md`
  (#1735) will close the door on arbitrary sample counts.
