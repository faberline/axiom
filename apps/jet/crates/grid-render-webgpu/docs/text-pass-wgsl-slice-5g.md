# text_pass.wgsl — Slice 5g

> Issue: #1756 · Parent epic: #1700 · Slice: 5g

## Problem

Slices 5a–5f delivered the *resources* for the text pass: font handles,
the rasterizer, the CPU cache, the GPU atlas descriptor, and the upload
glue. What's missing is the **shader** that turns a per-glyph
`GlyphInstance` (screen-space quad + atlas-space UV rect + RGBA color)
into colored pixels on screen by sampling the atlas's `R8Unorm` alpha
and multiplying by the per-glyph color.

This slice mirrors `cell_rect.wgsl` for glyphs. Four design choices pin
the shape:

1. **`@group(0) @binding(0)` viewport uniform**, byte-identical to the
   one `cell_rect.wgsl` already uses (`size_px` + `scroll_px`, 16
   bytes). Sharing the layout means the renderer can reuse the same
   viewport bind-group across both passes — one upload, two consumers.
2. **`@group(0) @binding(1)` atlas texture + `@binding(2)` linear
   sampler.** Slice 5h will build the bind-group layout; Slice 5i will
   build the sampler descriptor. The shader pins the *binding numbers*
   so those two slices have a fixed contract to author against. Atlas
   is `texture_2d<f32>` because `R8Unorm` reads expand to a normalized
   `f32` on shader sample.
3. **4-vertex triangle strip via `@builtin(vertex_index)`** — the same
   bit-trick as `cell_rect.wgsl` (`vid & 1`, `(vid >> 1) & 1`). No
   per-vertex buffer, only the per-instance buffer. Keeps the
   pipeline's draw call shape identical to the cell-rect pass so the
   render-pass orchestrator (future slice) can switch between them
   without re-binding a vertex buffer.
4. **`fs_main = textureSample(atlas, samp, uv).r * color_rgba`** —
   alpha lives in the red channel (because the atlas is `R8Unorm`,
   single-channel). Multiplying by `color_rgba` straight (not
   premultiplied) matches the cell-rect pass's blend mode
   (`ALPHA_BLENDING`), which expects un-premultiplied input. A future
   slice can introduce premultiplied output if profiling demands.

The per-instance struct carries six values:

- `pos_px:  vec2<f32>` — top-left of the glyph quad in virtual-sheet
  pixels.
- `size_px: vec2<f32>` — quad width/height in pixels.
- `uv_min:  vec2<f32>` — atlas-space top-left in *normalized* UV
  coordinates (0..1).
- `uv_max:  vec2<f32>` — atlas-space bottom-right in normalized UVs.
- `color:   vec4<f32>` — straight RGBA, 0..=1.

Why normalized UVs and not pixel coordinates? `textureSample` takes
normalized UVs natively; pixel coordinates would force a per-fragment
divide by the atlas dimensions. The caller (atlas allocator, future
slice) computes the division once per glyph at upload time.

## Scope

In:

- New module `jet_grid_render_webgpu::text_pass` with:
  - `pub const TEXT_PASS_WGSL: &str` — the shader source.
  - `pub struct GlyphInstance` — `#[repr(C)]` mirror of the WGSL
    `GlyphInstance` struct, with `Pod + Zeroable + Copy + Debug + PartialEq`.
    Fields in declaration order (per the WGSL → Rust contract).
- Module-level docs explain the WHY:
  - Why bindings 0/1/2 land where they do (viewport reuse from
    cell_rect, fixed contract for Slices 5h + 5i).
  - Why straight RGBA multiplication and not premultiplied.
  - Why normalized UVs in the per-instance struct.
  - Why no per-vertex buffer (`@builtin(vertex_index)` bit-trick).
- Unit tests pinning every AC bullet:
  - `wgsl_parses_via_naga` — `TEXT_PASS_WGSL` parses cleanly through
    naga's WGSL frontend (the same path wgpu uses internally).
  - `entry_points_are_vs_main_and_fs_main` — module exposes both
    contracted entry points.
  - `bindings_match_ac` — group 0 bindings 0, 1, 2 are declared in the
    parsed module's global vars.
  - `glyph_instance_layout` — `repr(C)` size, field offsets pinned.

Out:

- Bind-group layout descriptor (Slice 5h).
- Sampler descriptor (Slice 5i).
- Vertex-buffer layout / pipeline-config (sibling, follows Slice 5h's
  bind-group shape).
- Per-glyph color interpolation across the quad. The four vertices
  share one color; the fragment shader does not interpolate it
  pixel-by-pixel except by trivial pass-through.
- Subpixel positioning. The vertex shader uses
  `pos_px + corner * size_px` exactly — fractional pixels are pushed
  into atlas-side fractional UVs at layout time.
- Color-glyph (sRGB / Rgba8) atlas. Future slice; this one is alpha-only.

## Interface

```rust
/// WGSL source for the text pass (entry points `vs_main` / `fs_main`).
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/text-pass-wgsl-slice-5g.md#interface
/// @issue #1756
pub const TEXT_PASS_WGSL: &str = /* see source */;

/// Per-instance attributes for one glyph quad.
///
/// Field order matches the WGSL `GlyphInstance` struct; changing it
/// requires updating both sides in lockstep.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/text-pass-wgsl-slice-5g.md#interface
/// @issue #1756
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct GlyphInstance {
    pub pos_px:  [f32; 2],
    pub size_px: [f32; 2],
    pub uv_min:  [f32; 2],
    pub uv_max:  [f32; 2],
    pub color:   [f32; 4],
}
```

## Acceptance Criteria

- [x] `shader/text_pass.wgsl + pub const TEXT_PASS_WGSL` — shader is
      embedded directly in the Rust module (matches `CELL_RECT_WGSL`'s
      shape; no separate `shader/` directory in this crate). Source
      lives in `text_pass.rs`.
- [x] `@group(0) @binding(0)` viewport uniform — declared.
- [x] `@group(0) @binding(1)` atlas texture, `@binding(2)` sampler —
      declared.
- [x] `vs_main` expands 4-vertex triangle strip — bit-trick
      `(vid & 1, (vid >> 1) & 1)`, matching `cell_rect.wgsl`.
- [x] `fs_main`: `textureSample(atlas, samp, uv).r * color_rgba` —
      pinned.
- [x] `cargo test -p jet-grid-render-webgpu` passes.
- [x] Module-level docs explain the WHY (binding-number contract,
      straight-RGBA choice, normalized-UV choice, no-per-vertex-buffer
      bit-trick), not just the what.

## Reference Context

- Parent epic [#1700](https://github.com/chrischeng-c4/cclab/issues/1700) — text pass.
- Slice 4b (#1720) — `cell_rect.wgsl` + `CellInstance`. This module
  intentionally mirrors that one's shape (viewport binding, bit-trick
  vertex expansion, `#[repr(C)]` instance struct + `Pod`).
- Slice 5e (#1754) — `glyph_atlas_texture_descriptor`. The atlas
  the shader samples.
- Slice 5h (#1757) — text-pass bind-group layout. Consumes the
  binding numbers this slice pins.
- Slice 5i (#1758) — atlas sampler descriptor (linear, clamp). Consumes
  binding 2.
