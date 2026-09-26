# enhancement(jet): cell-rect pipeline construction — Slice 4b

> Issue: #1720 · Parent epic: #1699 (WebGPU render runtime) · Slice: 4b
>
> Spec location note: this lives next to the crate (`apps/jet/crates/grid-render-webgpu/docs/`)
> rather than `.aw/tech-design/projects/jet/` because the host worktree
> denies writes under `.aw/`. Co-locating the spec with the crate it
> describes is arguably cleaner anyway — readers find it via the crate.

## Problem

The Slice 4a wrapper (`WebGpuRenderer`) owns the GPU but has nothing to
draw with. Slice 4b composes the foundation primitives (cell-rect WGSL
shader, vertex layout, bind-group layout, pipeline config) into a real
`wgpu::RenderPipeline` and caches it per surface format. Later runtime
slices (frame loop, render-pass orchestration) consume this pipeline
to actually rasterise grid cells.

The primitives the epic (#1699) said "already exist in
jet-grid-render-webgpu" do **not** in fact exist yet — the crate was
created by #1719 with only the wrapper. So Slice 4b owns both
introducing the primitives in their minimum-viable form **and** the
pipeline composition. Later slices may evolve the primitives; the
contract here is just enough to render solid-coloured cells.

## Scope

In:
- A `cell_rect` module owning the WGSL source, the vertex/instance
  layout, and the bind-group layout descriptor.
- `WebGpuRenderer::create_cell_pipeline(format) -> Arc<RenderPipeline>`
  that builds the pipeline on first call and returns the cached entry
  on subsequent calls.
- A `HashMap<TextureFormat, Arc<RenderPipeline>>` cache field on
  `WebGpuRenderer`.
- Unit tests that validate the WGSL parses via `naga` (no live device
  required) and exercise the bind-group / vertex-layout descriptors
  structurally.

Out:
- Frame loop, render-pass orchestration, instance-buffer pool — these
  are sibling slices of #1699.
- Glyph atlas / text pass — those belong to #1700.
- Camera / DPR / viewport-uniform write-through — sibling slices.
- Pipeline cache eviction strategy — current design is grow-only,
  bounded by the small number of surface formats in use.

## Interface

```rust
// apps/jet/crates/grid-render-webgpu/src/cell_rect.rs

/// WGSL source for the cell-rect shader (vs_main, fs_main).
pub const CELL_RECT_WGSL: &str;

/// Single-cell instance attributes, fed via a per-instance vertex buffer.
/// `pos_px` is the top-left in physical pixels; `size_px` is width/height;
/// `color` is straight (un-premultiplied) RGBA in 0..=1.
#[repr(C)]
pub struct CellInstance {
    pub pos_px:  [f32; 2],
    pub size_px: [f32; 2],
    pub color:   [f32; 4],
}

/// `BindGroupLayoutDescriptor` for the single viewport uniform binding.
pub fn cell_rect_bind_group_layout_descriptor()
    -> wgpu::BindGroupLayoutDescriptor<'static>;

/// `VertexBufferLayout` for `CellInstance` (one buffer, step = Instance).
pub fn cell_rect_vertex_layout() -> wgpu::VertexBufferLayout<'static>;

/// Pipeline shared config (primitive topology = TriangleStrip with 4
/// vertices/instance; alpha-blend over; no depth/stencil).
pub fn cell_rect_pipeline_config() -> PipelineConfig;

pub struct PipelineConfig {
    pub primitive: wgpu::PrimitiveState,
    pub blend:     wgpu::BlendState,
    pub vertices_per_instance: u32,  // 4
}
```

```rust
// apps/jet/crates/grid-render-webgpu/src/lib.rs

impl<'window> WebGpuRenderer<'window> {
    /// Build (or fetch cached) cell-rect pipeline for `format`.
    /// One create per unique format; calls are idempotent.
    pub fn create_cell_pipeline(
        &mut self,
        format: wgpu::TextureFormat,
    ) -> std::sync::Arc<wgpu::RenderPipeline>;
}
```

The WGSL shader produces a textured quad per instance using
`@builtin(vertex_index)` bit-tricks to derive corners 0..3 → (0,0)
(1,0)(0,1)(1,1) — so no per-vertex vertex buffer is needed; only the
instance buffer is bound.

## Acceptance Criteria

- [x] `WebGpuRenderer::create_cell_pipeline(format) -> Arc<RenderPipeline>`
- [x] `ShaderModule` is built lazily on first `create_cell_pipeline`
      call and cached on the renderer for subsequent format builds.
- [x] `BindGroupLayout` is created from
      `cell_rect_bind_group_layout_descriptor()`.
- [x] `PipelineLayout` binds the BGL.
- [x] `RenderPipelineDescriptor` wires vs_main / fs_main from
      `CELL_RECT_WGSL` plus `cell_rect_vertex_layout()` and
      `cell_rect_pipeline_config()`.
- [x] Pipelines are cached on `WebGpuRenderer` (one create per surface
      format); calling `create_cell_pipeline` twice with the same
      format returns the same `Arc` (pointer equality, verifiable).
- [x] `cargo test -p jet-grid-render-webgpu` passes (naga-validated
      WGSL + descriptor shape tests).
- [x] Module-level docs explain the WHY (single-source-of-truth for the
      cell-rect render contract; later slices plug a frame loop on top
      of `create_cell_pipeline` rather than re-declaring layouts).

## Reference Context

- Parent epic: #1699 (WebGPU render runtime)
- Slice 4a (wrapper, merged): #1719 / PR #1904 — see
  `apps/jet/crates/grid-render-webgpu/src/lib.rs` for the `WebGpuRenderer`
  type this slice extends.
- Sibling slices (planned): cell-instance buffer pool, viewport uniform
  write-through, render-pass orchestration.
- `naga` parser is used in tests to validate the WGSL compiles without
  pulling in a real GPU adapter.
