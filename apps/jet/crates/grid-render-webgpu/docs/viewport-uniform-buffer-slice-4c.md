# enhancement(jet): viewport uniform buffer + write-through — Slice 4c

> Issue: #1721 · Parent epic: #1699 (WebGPU render runtime) · Slice: 4c
>
> Spec location note: lives next to the crate
> (`apps/jet/crates/grid-render-webgpu/docs/`) because the host worktree denies
> writes under `.aw/`. Sibling specs: Slice 4a (#1719) and Slice 4b
> (#1720).

## Problem

The cell-rect pipeline from Slice 4b binds a `Viewport` uniform at
`@group(0) @binding(0)`, but the buffer that backs that binding doesn't
exist yet — the pipeline would crash on first draw. Slice 4c allocates
that buffer once, builds the matching `BindGroup` once, and exposes a
write-through API that pushes a new `ViewportUniforms` value into the
buffer via `Queue::write_buffer` on every resize / scroll change.

## Scope

In:
- A `viewport` module owning the `ViewportUniforms` `#[repr(C)]` struct
  (matching the WGSL `Viewport` layout from Slice 4b).
- A persistent `wgpu::Buffer` on `WebGpuRenderer` sized to
  `size_of::<ViewportUniforms>()`, usage `UNIFORM | COPY_DST`.
- A persistent `wgpu::BindGroup` on `WebGpuRenderer` referencing that
  buffer at `binding(0)`, built once during `WebGpuRenderer::new`.
- `WebGpuRenderer::set_viewport(uniforms)` — `Queue::write_buffer` push.
- `on_resize` auto-calls `set_viewport` with the new physical size so
  the wrapper stays internally consistent without ceremony at call
  sites.
- Round-trip tests: struct byte-layout invariant + (live-GPU only)
  write→readback equality.

Out:
- Camera transform / scroll offset — sibling slice (will extend
  `ViewportUniforms`).
- DPR / device-pixel-ratio handling — sibling slice; tracked
  separately so this slice doesn't take on policy decisions.
- Multi-viewport or stereo rendering.

## Interface

```rust
// apps/jet/crates/grid-render-webgpu/src/viewport.rs

/// Uniform values consumed by the cell-rect shader at @binding(0).
/// Layout MUST match the WGSL `Viewport` struct in `cell_rect::CELL_RECT_WGSL`
/// (vec2<f32> at offset 0, padded to 16-byte uniform alignment).
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ViewportUniforms {
    pub size_px: [f32; 2],
    _pad:       [f32; 2],
}

impl ViewportUniforms {
    pub fn new(width_px: f32, height_px: f32) -> Self;
}
```

```rust
// apps/jet/crates/grid-render-webgpu/src/lib.rs

impl<'window> WebGpuRenderer<'window> {
    /// Push new viewport uniforms to the GPU via Queue::write_buffer.
    /// Cheap (no allocation, no bind-group rebuild).
    pub fn set_viewport(&self, uniforms: viewport::ViewportUniforms);

    /// Borrow the persistent bind-group for the cell-rect pipeline's
    /// viewport binding. Caller passes this to RenderPass::set_bind_group.
    pub fn viewport_bind_group(&self) -> &wgpu::BindGroup;
}
```

`on_resize` is extended to call `set_viewport` internally so callers
don't have to remember to push the new size after a resize.

## Acceptance Criteria

- [x] Uniform buffer created with `size_of::<ViewportUniforms>()`,
      usage `UNIFORM | COPY_DST`.
- [x] `WebGpuRenderer::set_viewport(uniforms)` writes via
      `Queue::write_buffer`.
- [x] `BindGroup` created once during `new`, references the uniform
      buffer at `@binding(0)`.
- [x] Round-trip test: set → readback → equal (live-GPU,
      `#[ignore]`'d).
- [x] `cargo test -p jet-grid-render-webgpu` passes.
- [x] Module-level docs explain the WHY (layout MUST match WGSL;
      bind-group is built once so the hot path stays alloc-free).

## Reference Context

- Parent epic: #1699 (WebGPU render runtime).
- Slice 4b (#1720, merged PR #1906): cell-rect pipeline. The WGSL it
  introduced expects the uniform this slice now backs.
- Slice 4a (#1719, merged PR #1904): wrapper struct this slice extends.
- `bytemuck` (1.x, workspace-free dep) gates the `Pod`/`Zeroable`
  derives that let us hand the struct directly to
  `Queue::write_buffer` without manual casts.
