# glyph_atlas_sampler_descriptor — Slice 5i

> Issue: #1758 · Parent epic: #1700 · Slice: 5i

## Problem

Slice 5h (#1757) declared binding 2 of the text-pass bind group as a
`Sampler(Filtering)`. This slice supplies the **concrete sampler
descriptor** the renderer uses when creating that sampler — linear
mag/min filter, clamp-to-edge addressing on every axis.

Two choices pin the descriptor:

1. **Linear mag + min filter.** Glyph sample positions rarely land
   exactly on atlas texel centers — subpixel positioning and
   non-integer pixel ratios are normal in a DPR-aware renderer. Linear
   filtering across the four neighbouring texels gives a smooth edge;
   nearest-neighbour would alias visibly on a fractional-pixel offset.
   The cost is one texture fetch's worth of bilinear lerp — trivial
   for an `R8Unorm` atlas. Slice 5h's bind-group entry already
   pre-committed us: a `Filtering` sampler requires the texture entry
   to be filterable, which means the sampler MUST be linear here. (A
   `NonFiltering` sampler would be `Nearest`-only and would pair with
   a `filterable: false` texture binding.)
2. **`ClampToEdge` on `address_mode_u/v/w`.** Glyphs are packed
   contiguously in the atlas — adjacent glyphs share atlas-texel
   boundaries. Any sample outside `[0, 1]` UV bleeds into a neighbour
   and prints a sliver of the wrong glyph at the edge of a quad. The
   safe behaviour is to clamp to the last in-rect texel and let the
   linear filter blend against that fixed boundary; `Repeat` would
   wrap and pull in the *opposite* edge of the atlas, `MirrorRepeat`
   would pull in the same edge mirrored. `ClampToEdge` is the only
   addressing mode that doesn't reach into a packed neighbour. The
   third axis (`w`) is meaningless for `D2` but wgpu requires a value;
   `ClampToEdge` is the conservative pick.

Mip / LOD / anisotropy / compare: defaults.

- `mipmap_filter = Nearest` — the atlas is `mip_level_count = 1`
  (Slice 5e), so there is nothing to filter between. Any value works;
  `Nearest` is the cheapest, and wgpu's `Default` already picks it.
- `lod_min_clamp = 0.0`, `lod_max_clamp = 0.0` — single mip level
  means LOD is fixed at 0.
- `compare = None` — this is a sampling sampler, not a depth-compare
  sampler.
- `anisotropy_clamp = 1` — anisotropic filtering trades GPU cost for
  glancing-angle quality; alpha glyphs at near-orthographic camera
  angles get no benefit. `1` disables it.
- `border_color = None` — irrelevant under `ClampToEdge`.

Label is `'static` `Some("glyph_atlas_sampler")` so the return type can
be `SamplerDescriptor<'static>` and GPU debuggers (RenderDoc, Xcode
capture) key off a stable name.

The function takes no arguments — the descriptor is invariant in width,
height, or any other runtime value. One sampler serves the entire
renderer.

## Scope

In:

- New item in `jet_grid_render_webgpu::glyph_atlas`:
  - `pub fn glyph_atlas_sampler_descriptor() -> wgpu::SamplerDescriptor<'static>`
    — pure function, hands the caller a descriptor ready for
    `device.create_sampler(&desc)`.
- Module-level docs (extend the existing Slice 5e doc) explain the WHY
  for `Linear` filtering and `ClampToEdge` addressing.
- Unit tests pinning every AC bullet:
  - `sampler_label_is_glyph_atlas_sampler` — label is the `'static`
    `Some("glyph_atlas_sampler")`.
  - `sampler_filters_are_linear` — `mag_filter` + `min_filter` are
    both `Linear`.
  - `sampler_address_modes_are_clamp_to_edge` — `u`, `v`, `w` all
    `ClampToEdge`.
  - `sampler_mip_lod_pinned_for_single_mip` — `lod_min_clamp = 0.0`,
    `lod_max_clamp = 0.0`, no anisotropy, no compare.

Out:

- Sampler *instance* construction (against a real device). Runtime
  concern; this slice is the descriptor shape.
- A second sampler (e.g. `Nearest` for pixel-perfect rendering modes).
  If a future slice needs it, the natural shape is a sibling
  `glyph_atlas_pixel_sampler_descriptor`. Out of scope now.
- Anisotropic / mip-filtered samplers for color-glyph atlases. Future.

## Interface

```rust
/// Build the [`wgpu::SamplerDescriptor`] used to sample the glyph
/// atlas. `Linear` mag/min filter + `ClampToEdge` addressing on every
/// axis. See module docs for the WHY behind those two choices.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/glyph-atlas-sampler-descriptor-slice-5i.md#interface
/// @issue #1758
pub fn glyph_atlas_sampler_descriptor() -> wgpu::SamplerDescriptor<'static>;
```

## Acceptance Criteria

- [x] `glyph_atlas_sampler_descriptor() -> SamplerDescriptor<'static>`
      — implemented; pure function, `'static` label.
- [x] `mag_filter = Linear`, `min_filter = Linear` — pinned.
- [x] `address_mode_u/v/w = ClampToEdge` — pinned.
- [x] Tests pin label, filter, address modes — and LOD/anisotropy/
      compare for completeness.
- [x] `cargo test -p jet-grid-render-webgpu` passes.
- [x] Module-level docs explain the WHY (Linear over Nearest for
      subpixel sampling, ClampToEdge to avoid bleed across packed
      glyphs), not just the what.

## Reference Context

- Parent epic [#1700](https://github.com/chrischeng-c4/cclab/issues/1700) — text pass.
- Slice 5e (#1754) — `glyph_atlas_texture_descriptor`. The atlas this
  sampler reads. `mip_level_count = 1` is why the LOD clamps land at
  `(0.0, 0.0)`.
- Slice 5g (#1756) — `TEXT_PASS_WGSL` binds the sampler at
  `@group(0) @binding(2)` and calls `textureSample(atlas, samp, uv)`.
- Slice 5h (#1757) — text-pass bind-group layout, entry 2 is
  `Sampler(Filtering)`. A `Filtering` sampler binding *requires* the
  underlying sampler to be linear; if a future slice flips this to
  `NonFiltering`, the sampler here would need to flip to `Nearest`
  in lockstep.
- `wgpu 24` `SamplerDescriptor` defaults — `Default` returns
  `Linear/Linear/Nearest` filters + `ClampToEdge` addressing. We name
  every field explicitly so a future wgpu rev that changes a default
  doesn't silently shift our behaviour.
