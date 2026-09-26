# TEXT_PASS_BIND_GROUP_ENTRIES — Slice 5h

> Issue: #1757 · Parent epic: #1700 · Slice: 5h

## Problem

Slice 5g (#1756) pinned the WGSL binding numbers
(`@group(0) @binding(0/1/2)` for viewport / atlas / sampler). This
slice adds the **Rust-side bind-group layout entries** so the pipeline
builder can construct one `BindGroupLayout` that matches the shader's
contract exactly.

The layout has three entries, each with constraints fixed by what the
shader does:

1. **Entry 0 — viewport uniform.** Visible to `VERTEX` only (the shader
   uses `viewport.size_px` + `viewport.scroll_px` inside `vs_main`,
   never inside `fs_main`). `has_dynamic_offset: false` because the
   renderer binds one uniform buffer per frame, not a sliding window
   into a larger pool. `min_binding_size: None` because the buffer
   shape is pinned by the cell-rect pass already (16 bytes); a future
   layout-validation slice can tighten this.
2. **Entry 1 — atlas texture.** Visible to `FRAGMENT` only (`fs_main`
   calls `textureSample(atlas, ...)`). `sample_type:
   TextureSampleType::Float { filterable: true }` because the atlas is
   `R8Unorm` (8-bit unsigned normalized → `f32` on read) **and** the
   sampler is `Filtering` (entry 2), which requires the texture to be
   filterable. `view_dimension: D2` matches Slice 5e's
   `TextureDimension::D2`. `multisampled: false` matches Slice 5e's
   `sample_count = 1`. The AC mentions "R8Unorm view dim D2" — wgpu's
   `BindGroupLayoutEntry` does not carry the format directly (the
   *texture* descriptor does); the layout pins `sample_type` +
   `view_dimension` + `multisampled` to match.
3. **Entry 2 — sampler.** Visible to `FRAGMENT` only. `Filtering`
   (linear/bilinear) — Slice 5i will pin the concrete
   `SamplerDescriptor` to `Linear` mag/min filters with `ClampToEdge`
   addressing. `Filtering` here is the *binding-type* category; it
   requires the texture entry (1) to be filterable, which we already
   set.

Why visibility split (vertex vs. fragment)? wgpu's validation rejects a
binding that names a shader stage the shader doesn't actually use at
that binding. Naming `VERTEX | FRAGMENT` on the texture entry would
pass at first but trip `wgpu_core` validation as soon as a shader edit
removes the corresponding read. The minimum-surface approach surfaces
shader/layout drift loud.

Why a `pub const` array, not a function returning `BindGroupLayoutDescriptor`?
The cell-rect pass uses the same convention
(`const VIEWPORT_BGL_ENTRIES`), and the pipeline builder will compose
`BindGroupLayoutDescriptor { label: Some("text_pass_bgl"), entries:
&TEXT_PASS_BIND_GROUP_ENTRIES }` at construction time. A `const` keeps
the entries `'static`-borrowable and lets the cell-rect module's
`VIEWPORT_BGL_ENTRIES` precedent stand unchanged. We also expose a
`pub fn text_pass_bind_group_layout_descriptor()` that wraps the const
into the full descriptor — same shape as
`cell_rect_bind_group_layout_descriptor`.

## Scope

In:

- New items in `jet_grid_render_webgpu::text_pass`:
  - `pub const TEXT_PASS_BIND_GROUP_ENTRIES: [wgpu::BindGroupLayoutEntry; 3]`
    — the three layout entries in binding-number order.
  - `pub fn text_pass_bind_group_layout_descriptor() -> wgpu::BindGroupLayoutDescriptor<'static>`
    — composes the const with a `'static` label, ready for
    `device.create_bind_group_layout(&desc)`.
- Module-level docs (extend the existing Slice 5g doc) explain the WHY
  for the visibility split, the `Float { filterable: true }` choice,
  and the `Filtering` sampler binding category.
- Unit tests pinning every AC bullet:
  - `bind_group_entries_count_is_three` — the const has length 3.
  - `entry_zero_is_viewport_uniform_vertex` — binding 0,
    `ShaderStages::VERTEX`, `BufferBindingType::Uniform`, no dynamic
    offset.
  - `entry_one_is_atlas_texture_fragment_d2` — binding 1,
    `ShaderStages::FRAGMENT`, `BindingType::Texture` with
    `TextureSampleType::Float { filterable: true }`,
    `TextureViewDimension::D2`, `multisampled: false`.
  - `entry_two_is_sampler_fragment_filtering` — binding 2,
    `ShaderStages::FRAGMENT`,
    `BindingType::Sampler(SamplerBindingType::Filtering)`.
  - `descriptor_carries_static_label` — descriptor's `label` is
    `Some("text_pass_bgl")`, `'static`.

Out:

- Pipeline-config / `RenderPipelineDescriptor`. Composed by the
  renderer's pipeline builder; sibling concern.
- Sampler descriptor (Slice 5i — linear, clamp).
- Vertex-buffer layout for `GlyphInstance`. The shader's per-instance
  attributes (`@location(0..4)`) drive a separate
  `vertex_attr_array![...]` — a sibling slice once the pipeline
  builder lands.
- `BindGroup` *instance* construction (textures bound to a real device).
  That's a runtime concern; this slice is the layout shape.

## Interface

```rust
/// Bind-group layout entries for the text pass — viewport (vertex
/// uniform), atlas (fragment texture), sampler (fragment filtering).
///
/// Pinned in binding-number order so a future composer can index by
/// position. See module docs for the WHY behind the visibility split
/// and the `Float { filterable: true }` choice.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/text-pass-bind-group-layout-slice-5h.md#interface
/// @issue #1757
pub const TEXT_PASS_BIND_GROUP_ENTRIES: [wgpu::BindGroupLayoutEntry; 3] = [
    /* viewport, atlas, sampler */
];

/// `BindGroupLayoutDescriptor` composing [`TEXT_PASS_BIND_GROUP_ENTRIES`]
/// with the `'static` label `"text_pass_bgl"`.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/text-pass-bind-group-layout-slice-5h.md#interface
/// @issue #1757
pub fn text_pass_bind_group_layout_descriptor() -> wgpu::BindGroupLayoutDescriptor<'static>;
```

## Acceptance Criteria

- [x] `TEXT_PASS_BIND_GROUP_ENTRIES` const — declared, length 3.
- [x] Entry 0: `Uniform`, `VERTEX` — pinned.
- [x] Entry 1: `Texture`, `FRAGMENT`, `view_dimension: D2`, sample type
      matches the R8Unorm filterable contract.
- [x] Entry 2: `Sampler(SamplerBindingType::Filtering)`, `FRAGMENT` —
      pinned.
- [x] `cargo test -p jet-grid-render-webgpu` passes.
- [x] Module-level docs explain the WHY (visibility split, filterable
      choice), not just the what.

## Reference Context

- Parent epic [#1700](https://github.com/chrischeng-c4/cclab/issues/1700) — text pass.
- Slice 5e (#1754) — atlas texture descriptor (R8Unorm, D2,
  sample_count=1). This layout's texture entry must match.
- Slice 5g (#1756) — `TEXT_PASS_WGSL` pinned the binding numbers
  this layout consumes.
- Slice 5i (#1758) — sampler descriptor (linear, clamp). Pairs with
  this slice's `Filtering` sampler entry.
- `wgpu 24` `BindGroupLayoutEntry` — texture entries carry
  `sample_type` + `view_dimension` + `multisampled`, not the texture
  *format* directly. Format is implicit through `sample_type`.
