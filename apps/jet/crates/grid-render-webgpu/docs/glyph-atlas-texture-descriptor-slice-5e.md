# glyph_atlas_texture_descriptor — Slice 5e

> Issue: #1754 · Parent epic: #1700 · Slice: 5e

## Problem

The text pass needs a single texture on the GPU that holds every
rasterized glyph as alpha pixels — the "glyph atlas". Slice 5d
(#1753) delivered the CPU-side cache; this slice locks the
**GPU-side texture shape** in one place so the upload (Slice 5f),
the bind group (Slice 5h), and the sampler (Slice 5i) all see the
same dimensions / format / usage flags.

Three constraints pin the descriptor:

1. **`R8Unorm`**, not `R8Snorm` / `Rg8` / `Rgba8`. Slice 5c's
   rasterizer produces 8-bit grayscale alpha — a single channel
   per pixel. `R8Unorm` matches one-for-one (`u8` → `[0.0, 1.0]`
   on read), is supported on every wgpu backend the renderer
   targets (Metal, Vulkan, D3D12, OpenGL ES via
   `TEXTURE_FORMAT_R8_UNORM_STORAGE` not required), and is 4×
   smaller in VRAM than RGBA. Future color-glyph slices can add
   a sibling `Rgba8UnormSrgb` atlas; that's not this slice.
2. **`COPY_DST | TEXTURE_BINDING`** usage. `COPY_DST` is what
   `queue.write_texture` writes through (Slice 5f); `TEXTURE_BINDING`
   is what the text-pass bind group binds (Slice 5h). No
   `RENDER_ATTACHMENT` (we don't draw INTO the atlas), no
   `STORAGE_BINDING` (shader writes aren't part of this pipeline).
3. **`mip_level_count = 1`, `sample_count = 1`, `dimension = D2`**.
   Glyph rendering uses the bitmap at exactly one mip level (the
   atlas slot's natural size); multisampling on an alpha lookup
   would burn memory for no quality benefit; the third dimension
   is meaningless for a 2D atlas.

The function takes `(width, height)` so the same descriptor builder
serves both the initial atlas allocation and any future resize.
Label is fixed `Some("glyph_atlas")` — a `'static` string so the
return type can be `TextureDescriptor<'static>` per the AC.

## Scope

In:

- New module `jet_grid_render_webgpu::glyph_atlas` with:
  - `pub fn glyph_atlas_texture_descriptor(width: u32, height: u32) -> wgpu::TextureDescriptor<'static>`
    — pure function, no I/O, no GPU calls. Hands the caller a
    descriptor ready for `device.create_texture(&desc)`.
- Module-level docs explain the WHY:
  - Why `R8Unorm` over the alternatives (memory + format
    compatibility with the rasterizer).
  - Why `COPY_DST | TEXTURE_BINDING` and no other usage flags
    (upload + bind, no draw / no compute).
  - Why mip = 1, sample = 1, D2 (no benefit in this layer).
- Unit tests pinning every AC bullet, plus label / view_formats.

Out:

- Atlas allocator (which sub-rect each glyph lands in). Future
  slice (probably 5f or sibling).
- The actual `Texture` / `TextureView` — that's a runtime concern;
  this slice is the descriptor shape.
- Bind-group layout / sampler — Slices 5h and 5i.
- Resize on overflow / repacking. Future slice.
- Multi-atlas support (per-size-bucket atlases). Future slice if
  packing efficiency demands it.

## Interface

```rust
/// Build the `wgpu::TextureDescriptor` for the glyph atlas at a
/// given `(width, height)`. See module docs for the WHY behind the
/// `R8Unorm` + `COPY_DST | TEXTURE_BINDING` choices.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/glyph-atlas-texture-descriptor-slice-5e.md#interface
/// @issue #1754
pub fn glyph_atlas_texture_descriptor(
    width: u32,
    height: u32,
) -> wgpu::TextureDescriptor<'static>;
```

## Acceptance Criteria

- [x] `glyph_atlas_texture_descriptor(width, height) -> TextureDescriptor<'static>` —
      implemented; pure function, `'static` label.
- [x] `format = R8Unorm` — pinned by unit test.
- [x] `usage = COPY_DST | TEXTURE_BINDING` — pinned by unit test.
- [x] `mip_level_count = 1`, `sample_count = 1` — pinned.
- [x] `dimension = D2` — pinned.
- [x] `cargo test -p jet-grid-render-webgpu` passes.
- [x] Module-level docs explain the WHY (format choice, usage flag
      choice, mip / sample / dimension rationale), not just the what.

## Reference Context

- Parent epic [#1700](https://github.com/chrischeng-c4/cclab/issues/1700) — text pass
  (glyph rasterization, shaping, atlas upload, font fallback).
- Slice 5c (#1752) — `rasterize_glyph` produces the 8-bit alpha
  bitmap whose format matches `R8Unorm`.
- Slice 5d (#1753) — `GlyphCache` holds the bitmaps on the CPU
  side; the atlas is the GPU-side mirror.
- Slice 5f (#1755) — `queue.write_texture` upload uses this
  descriptor's format + dimensions.
- Slice 5h (#1757) — text-pass bind-group layout consumes the
  `TEXTURE_BINDING` half of the usage flags.
- Slice 5i (#1758) — atlas sampler descriptor (linear, clamp).
- `wgpu 24` `TextureDescriptor` shape — `view_formats: &[]` is the
  default; we don't surface alternate views from this atlas.
