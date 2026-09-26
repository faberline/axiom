# shape_run — Slice 5j

> Issue: #1759 · Parent epic: #1700 · Slice: 5j

## Problem

Given a face, a string, and a pixel size, the text pass needs an
**ordered list of positioned glyphs** so the layout stage can hand each
glyph to the atlas allocator + the GPU. This slice delivers the
simplest useful shaper:

- Left-to-right only — no bidi (Hebrew/Arabic right-to-left runs are a
  future slice).
- No complex shaping — no GSUB/GPOS substitutions, no kerning beyond
  the font's horizontal advance, no Indic/SEA reordering, no
  emoji-cluster handling. The function walks the string `char`-by-`char`
  and maps each to one glyph.
- Honors the font's line metric — when the input contains `'\n'`, the
  cursor wraps: `x` resets to `0.0` and `y` advances by
  `(ascender - descender + line_gap) * size_px / units_per_em`.

Three constraints pin the output shape:

1. **`PositionedGlyph { glyph_id, x, y }` carries pixel-space
   coordinates as `f32`.** `f32` matches the WGSL `GlyphInstance` from
   Slice 5g (#1756) so the layout stage can copy directly into
   instance buffers without a per-glyph cast. Sub-pixel positions are
   real: a `hor_advance` of `512` design units at `size_px = 14`,
   `units_per_em = 1000` is exactly `7.168` pixels. The layout stage
   is the right place to decide whether to round or snap.
2. **Missing-glyph characters are silently skipped.** `glyph_index`
   returns `None` for codepoints the font doesn't cover; this slice
   does not synthesize a tofu rectangle. The text pipeline's fallback
   slice (epic backlog) will handle missing glyphs by trying a sibling
   face. Skipping here means the shaper output is always renderable
   against *this* face — never a `None` glyph_id that the GPU would
   choke on.
3. **`'\n'` is the only special character.** Carriage return, tab,
   vertical tab, and form-feed all flow as ordinary codepoints (most
   of which return `None` from `glyph_index` and are skipped). A real
   text editor needs richer whitespace handling; this slice does not.

Why `Vec<PositionedGlyph>` and not an iterator? The caller is the
atlas allocator + the instance-buffer uploader — both need random
access to glyphs in a run (e.g. to compute the bounding rect of the
text for layout fitting). An iterator would force a re-allocation at
the call site to materialize the same `Vec`; better to do it once
here.

Why does the function not return the line height separately? The
caller can compute it identically from `face.ascender() -
face.descender() + face.line_gap()` scaled by `size_px /
units_per_em`. Exposing it as a second return would surface the
formula twice (once internal, once in callers). If a future API ergonomics
slice wants a `ShapedRun { glyphs: Vec<...>, line_height_px: f32 }`,
that's a wrapper, not a redesign.

## Scope

In:

- New module `jet_grid_render_webgpu::shaper` with:
  - `pub struct PositionedGlyph { pub glyph_id: GlyphId, pub x: f32, pub y: f32 }` —
    `Copy + Debug + PartialEq`, pixel-space.
  - `pub fn shape_run(face: &FontFace, text: &str, size_px: u32) -> Vec<PositionedGlyph>` —
    deterministic, allocates once.
- Module-level docs explain the WHY:
  - Why ASCII-LTR-only is acceptable for this slice.
  - Why missing glyphs are skipped, not substituted.
  - Why `'\n'` is the only special character.
  - Why pixel-space `f32`, not design-unit `i32`.
- Unit tests:
  - `shape_run_hello_positions_match_advance_sum` — call with `'hello'`,
    assert glyph count = 5, `y == 0.0` for every glyph, `x[i]` equals
    the cumulative sum of `hor_advance` (scaled to pixels) for the
    preceding chars. Soft-skips if no system font is available.
  - `shape_run_newline_advances_y_by_line_height` — call with `'a\nb'`,
    assert `glyphs[0].y == 0.0`, `glyphs[1].y == line_height`,
    `glyphs[1].x == 0.0`. Soft-skips on missing font.
  - `shape_run_skips_missing_glyphs` — pure unit test against a
    deterministic in-memory test font; pass a codepoint the font
    doesn't cover and assert it's skipped silently. Soft-skips if
    `glyph_index` returns `Some` for every test codepoint we picked
    (true for most fonts).
  - `shape_run_empty_string` — empty input yields an empty `Vec`.
  - `positioned_glyph_value_object` — pin the pub-fields shape.

Out:

- Bidirectional layout (Unicode Bidi Algorithm). Future slice.
- Kerning beyond `hor_advance` (GPOS class kerning, pair kerning
  tables). Future.
- Complex shaping (GSUB substitutions, ligatures, Indic reordering).
  Future — likely depends on `rustybuzz` or `harfbuzz_rs` integration.
- Font fallback (sibling face for missing codepoints). Future.
- Word wrap / line breaking on width. Future layout slice.
- Tab expansion / control-character rendering. Future.
- Subpixel positioning / hinting modes. The shaper hands back exact
  `f32` positions; the layout stage decides rounding.

## Interface

```rust
/// A glyph + its baseline-anchored pixel position in a shaped run.
/// `x` is the glyph's pen-position (advance to draw from); `y` is the
/// baseline offset from the run's first-line baseline (0.0 for the
/// first line; line_height for the second, etc.).
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/shape-run-slice-5j.md#interface
/// @issue #1759
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PositionedGlyph {
    pub glyph_id: GlyphId,
    pub x: f32,
    pub y: f32,
}

/// Shape a `text` run against `face` at `size_px`. Left-to-right
/// ASCII-grade shaping: each `char` maps to one glyph via
/// `face.glyph_index`, `x` accumulates `hor_advance * scale`,
/// `'\n'` resets `x` to 0 and adds `(ascender - descender + line_gap)
/// * scale` to `y`. Missing-glyph characters are silently dropped.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/shape-run-slice-5j.md#interface
/// @issue #1759
pub fn shape_run(
    face: &FontFace,
    text: &str,
    size_px: u32,
) -> Vec<PositionedGlyph>;
```

## Acceptance Criteria

- [x] `PositionedGlyph { glyph_id, x, y }` — implemented as pub-fields
      value object.
- [x] `shape_run` honors line height = `ascender - descender + line_gap`
      — on `'\n'`, `y` advances by that quantity scaled to pixels;
      pinned by `shape_run_newline_advances_y_by_line_height`.
- [x] Test on `'hello'` against known-good metrics from a test font —
      `shape_run_hello_positions_match_advance_sum` cross-checks
      shaper output against the same `hor_advance` calls a hand-rolled
      shaper would make. (Soft-skips on workers without system fonts;
      matches the established `glyph_raster::tests::rasterize_a_glyph_round_trip`
      precedent.)
- [x] `cargo test -p jet-grid-render-webgpu` passes.
- [x] Module-level docs explain the WHY (ASCII-LTR scope, missing-
      glyph policy, `'\n'` policy, pixel-`f32` choice), not just the
      what.

## Reference Context

- Parent epic [#1700](https://github.com/chrischeng-c4/cclab/issues/1700) — text pass.
- Slice 5a (#1750) — `FontFace` API (`glyph_index`, `hor_advance`,
  `ascender`/`descender`/`line_gap`, `units_per_em`) — this slice's
  inputs.
- Slice 5b (#1751) — `FontDb::default_with_system_fonts` — used by
  the soft-skip live tests.
- Slice 5d (#1753) — `GlyphCache` keyed by `(face_id, glyph_id, size_px)`.
  The next step after `shape_run` is to feed each `PositionedGlyph`'s
  `glyph_id` through the cache to obtain a `GlyphEntry`.
- Slice 5g (#1756) — `GlyphInstance` carries `pos_px: [f32; 2]` in
  pixel space; `shape_run`'s `f32` positions flow into the first
  component directly.
