# rasterize_glyph(face, glyph_id, size_px) -> bitmap — Slice 5c

> Issue: #1752 · Parent epic: #1700 · Slice: 5c

## Problem

Slice 5a delivered [`FontFace`] (parsed-font handle); Slice 5b
delivered [`FontDb`] (family registry). Both produce a `(face,
glyph_id)` pair the rest of the text pipeline needs to turn into
pixels. This slice is the rasterizer: a function that takes a
[`FontFace`], a [`GlyphId`], and an integer pixel size, and produces
an 8-bit grayscale alpha bitmap plus the placement metrics the
caller needs to position it on the baseline.

Picking the backend: `fontdue` is the right fit here.

- `fontdue::Font::rasterize_indexed(glyph_id, px) -> (Metrics, Vec<u8>)`
  returns the bitmap and the metrics in one call, no outline walking
  required on our side.
- The bitmap layout (`Vec<u8>`, grayscale, top-left origin) is exactly
  the AC's literal signature — no copy / transpose.
- Pure Rust, no `unsafe`, builds on every host the renderer targets.

The alternatives — `swash` (shaping + raster, much bigger surface)
and `ab_glyph` (similar shape but requires us to translate `OutlineBuilder`
output to a bitmap) — are heavier than this slice's contract calls
for. If subpixel positioning or shaping becomes hot we revisit;
they're sibling slices, not this one's problem.

**Tradeoff documented in the Cargo.toml entry**: `fontdue 0.9` pulls
a private copy of `ttf-parser` (0.21) internally, alongside our
existing 0.25 pin. Duplicate is build-time only and small (~150 KB
in target/); accepting it is cheaper than forking fontdue to
expose its internal parser.

## Scope

In:

- New module `jet_grid_render_webgpu::glyph_raster` with:
  - `pub struct GlyphBitmap { bitmap: Vec<u8>, width: u32, height: u32, baseline_offset: i32, advance: f32 }`
    + accessors. All fields `pub` for ergonomic destructuring at the
    upload site; the struct is a value object, not an invariant
    holder.
  - `pub enum RasterError { ZeroSize, GlyphOutOfRange(u16), FontParseFailed(String) }`
    with `Display + std::error::Error`.
  - `pub fn rasterize_glyph(face: &FontFace, glyph_id: GlyphId, size_px: u32) -> Result<GlyphBitmap, RasterError>`.
    `Result` (not `Option`) so callers can tell the three failure
    modes apart — `ZeroSize` is a programmer bug, `GlyphOutOfRange`
    is a stale-id check, `FontParseFailed` is genuine font corruption.
- New dep on `fontdue = "0.9"` in `apps/jet/crates/grid-render-webgpu/Cargo.toml`.
- Module-level docs explain the WHY:
  - Why `fontdue` over `swash` / `ab_glyph` for the foundation slice.
  - **`baseline_offset` sign convention**: distance from the
    baseline to the **top** edge of the bitmap, in whole pixels,
    positive for glyphs that sit above the baseline. Derived from
    fontdue's `(ymin + height)`. Documented because every glyph
    library uses a subtly different convention and a wrong sign
    here silently produces upside-down text in the consumer.
  - **`advance`** is in subpixels (fractional pixels), copied
    straight from `Metrics::advance_width`. Layout code does the
    rounding when it commits a baseline X position.
- Unit tests:
  - `rasterize_zero_size_errors` — `size_px == 0` returns `ZeroSize`.
  - `rasterize_out_of_range_errors` — a glyph id beyond `glyph_count`
    returns `GlyphOutOfRange`.
  - `rasterize_a_glyph_round_trip` — embedded test font, rasterize
    'A' at 14 px, assert: non-empty bitmap, `width > 0`, `height > 0`,
    `advance > 0.0`, bitmap length == width * height.
  - `glyph_bitmap_value_object` — pin the struct's public fields so
    a refactor that hides a field fails here, not in production.
  - Error `Display` formats.
- A tiny fixture: vendor an embedded TTF under
  `apps/jet/crates/grid-render-webgpu/tests/fixtures/` (a 5–10 KB
  permissively licensed font) so the round-trip test doesn't depend
  on system fonts.

Out:

- Subpixel positioning. The AC says "Honors integer pixel sizes for
  now (no subpixel)". `fontdue::rasterize_indexed_subpixel` exists
  but a follow-up slice will wire it.
- Caching. Slice 5d (#1753) owns the glyph cache.
- Atlas / texture upload. Slices 5e (#1754) and 5f (#1755).
- Shaping. The function takes a pre-shaped `GlyphId`; shaping is
  a sibling slice on the epic.
- Color glyphs / SVG-in-OT / emoji. fontdue is monochrome-only;
  documented as a follow-up if the renderer ever needs emoji.

## Interface

```rust
/// 8-bit grayscale alpha bitmap of a single rasterized glyph, plus
/// the placement metrics needed to position it on a baseline.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/rasterize-glyph-bitmap-slice-5c.md#interface
/// @issue #1752
#[derive(Debug, Clone, PartialEq)]
pub struct GlyphBitmap {
    /// Linear grayscale alpha, top-left origin. `bitmap.len() ==
    /// (width * height) as usize` for every successfully rasterized
    /// glyph (including whitespace, which is empty).
    pub bitmap: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// Distance from the baseline to the **top** edge of the bitmap,
    /// in whole pixels. Positive for glyphs that sit above the
    /// baseline (the common case). Equal to fontdue's
    /// `ymin + height`. See module docs for the sign-convention WHY.
    pub baseline_offset: i32,
    /// Horizontal advance in **subpixels** (fractional pixels).
    /// Layout code rounds when it commits a baseline X position.
    pub advance: f32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RasterError {
    /// `size_px == 0` — programmer bug, would NaN fontdue's scale
    /// factor.
    ZeroSize,
    /// `glyph_id >= font.glyph_count()` — stale id from a previous
    /// font revision.
    GlyphOutOfRange(u16),
    /// `fontdue::Font::from_bytes` rejected the FontFace's bytes.
    /// Should not happen for a `FontFace` that already passed
    /// Slice 5a's validation, but documented for completeness.
    FontParseFailed(String),
}

pub fn rasterize_glyph(
    face: &FontFace,
    glyph_id: GlyphId,
    size_px: u32,
) -> Result<GlyphBitmap, RasterError>;
```

## Acceptance Criteria

- [x] `rasterize_glyph` returns (bitmap, width, height,
      `baseline_offset`, advance) — implemented as `GlyphBitmap`
      struct with `pub` fields so destructuring at the upload site
      is one line.
- [x] Bitmap is `Vec<u8>` grayscale — fontdue's native output.
- [x] Honors integer pixel sizes for now (no subpixel) — `size_px:
      u32`; subpixel variant deliberately deferred.
- [x] Round-trips a known glyph (e.g. 'A' at 14 px) — covered by
      `rasterize_a_glyph_round_trip` using the embedded fixture font.
- [x] `cargo test -p jet-grid-render-webgpu` passes.
- [x] Module-level docs explain the WHY (fontdue choice;
      baseline-offset sign convention; advance-in-subpixels), not
      just the what.

## Reference Context

- Parent epic [#1700](https://github.com/chrischeng-c4/cclab/issues/1700) —
  text pass (glyph rasterization, shaping, atlas upload, font fallback).
- Slice 5a (#1750) — [`FontFace`] handle this slice rasterizes from.
  Spec at `apps/jet/crates/grid-render-webgpu/docs/ttf-parser-font-face-wrapper-slice-5a.md`.
- Slice 5b (#1751) — [`FontDb`] registry that produces the `(face,
  glyph_id)` pairs callers will hand here. Spec at
  `apps/jet/crates/grid-render-webgpu/docs/font-db-load-and-index-slice-5b.md`.
- Sibling slices 5d (#1753, glyph cache), 5e (#1754, R8 atlas
  descriptor), 5f (#1755, atlas upload) — downstream consumers of
  the [`GlyphBitmap`] produced here.
- `fontdue 0.9.3` — `Font::rasterize_indexed(glyph_id, px)`. We
  reparse the `fontdue::Font` from `FontFace::bytes()` on every
  call for this slice; Slice 5d's cache will hold the parsed
  `fontdue::Font` to amortize.
