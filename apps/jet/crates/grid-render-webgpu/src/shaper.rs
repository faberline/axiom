//! Text shaper — Slice 5j (#1759).
//!
//! Converts a `(face, &str, size_px)` triple into an ordered list of
//! [`PositionedGlyph`]s ready for the atlas allocator + the GPU
//! instance-buffer uploader. This is the *simplest* useful shaper —
//! left-to-right, ASCII-grade, no kerning beyond the font's
//! `hor_advance`.
//!
//! ## Why ASCII-LTR-only is acceptable for this slice
//!
//! The text pass epic's first deliverable is a single-font ASCII grid
//! — the renderer needs to render strings like `"hello"` and `"42"`
//! correctly before it needs to render anything else. Adding bidi /
//! complex shaping at this stage means dragging in `rustybuzz` or
//! `harfbuzz_rs`, both of which compile non-trivially and make the
//! first end-to-end text frame less reachable. The fallback story is
//! a sibling slice once the rest of the pipeline is alive.
//!
//! ## Why missing-glyph characters are silently skipped
//!
//! [`FontFace::glyph_index`] returns `None` for codepoints the font
//! doesn't cover. Synthesizing a "tofu" rectangle would require
//! emitting a `PositionedGlyph` with a sentinel `glyph_id`, which the
//! GPU atlas allocator (future slice) would then need to special-case.
//! The text pipeline's *font fallback* slice (epic backlog) is the
//! right place to handle missing glyphs by trying a sibling face. For
//! now, dropping unknowns keeps the contract simple: the returned
//! `Vec<PositionedGlyph>` is always renderable against *this* face —
//! never carries a `None` glyph_id the GPU would choke on.
//!
//! ## Why `'\n'` is the only special character
//!
//! Carriage return, tab, vertical tab, and form-feed all flow as
//! ordinary codepoints (most return `None` from `glyph_index` and are
//! dropped by the missing-glyph rule). A real text editor needs
//! richer whitespace handling — tab stops, soft line breaks, etc. —
//! but the renderer's first text frame can ignore them. `'\n'` is
//! handled because the AC's line-height contract requires it.
//!
//! ## Why pixel-space `f32`, not design-unit `i32`
//!
//! [`super::text_pass::GlyphInstance::pos_px`] is `[f32; 2]`. Storing
//! shaper output as `f32` pixel-space means the layout stage can copy
//! coordinates directly into instance buffers — no per-glyph cast, no
//! rounding decision pushed up the stack. Sub-pixel positions are
//! real: `hor_advance` of `512` design units at `size_px = 14`,
//! `units_per_em = 1000` is exactly `7.168` pixels. The caller
//! decides whether to round or snap.

use crate::font_face::{FontFace, GlyphId};

/// A glyph + its baseline-anchored pixel position in a shaped run.
///
/// `x` is the pen-position to draw from (i.e. the run's left edge of
/// this glyph's quad, ignoring `baseline_offset` from the raster);
/// `y` is the baseline offset of the *line* this glyph sits on —
/// `0.0` for the first line, `line_height` for the second, etc.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/shape-run-slice-5j.md#interface
/// @issue #1759
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PositionedGlyph {
    pub glyph_id: GlyphId,
    pub x: f32,
    pub y: f32,
}

/// Shape `text` against `face` at `size_px`. Left-to-right
/// ASCII-grade — each `char` maps to one glyph via
/// [`FontFace::glyph_index`]; `x` accumulates the scaled
/// [`FontFace::hor_advance`]; `'\n'` resets `x` to `0.0` and adds
/// `(ascender - descender + line_gap) * size_px / units_per_em` to
/// `y`. Missing-glyph characters are silently dropped.
///
/// Allocates a single `Vec<PositionedGlyph>` and returns it. See
/// module docs for the WHY behind the no-kerning + no-fallback +
/// drop-unknown design.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/shape-run-slice-5j.md#interface
/// @issue #1759
pub fn shape_run(face: &FontFace, text: &str, size_px: u32) -> Vec<PositionedGlyph> {
    if text.is_empty() || size_px == 0 {
        return Vec::new();
    }
    let units_per_em = face.units_per_em();
    // FontFace::from_bytes already rejects `units_per_em == 0`, so a
    // valid face here always has a non-zero denominator. Belt-and-
    // suspenders for the divide.
    if units_per_em == 0 {
        return Vec::new();
    }
    let scale = size_px as f32 / units_per_em as f32;
    let line_height =
        (face.ascender() as f32 - face.descender() as f32 + face.line_gap() as f32) * scale;

    let mut glyphs = Vec::with_capacity(text.len());
    let mut x = 0.0f32;
    let mut y = 0.0f32;
    for c in text.chars() {
        if c == '\n' {
            x = 0.0;
            y += line_height;
            continue;
        }
        let Some(glyph_id) = face.glyph_index(c) else {
            continue;
        };
        glyphs.push(PositionedGlyph { glyph_id, x, y });
        if let Some(advance) = face.hor_advance(glyph_id) {
            x += advance as f32 * scale;
        }
    }
    glyphs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font_db::{FaceId, FontDb};

    /// Walk the system font registry and run `body` against the first
    /// face that covers every char in `required`. Returns `true` iff
    /// `body` ran. Used to soft-skip tests on workers that lack a
    /// usable font (matches the pattern in
    /// `glyph_raster::tests::rasterize_a_glyph_round_trip`).
    fn with_face_covering(required: &[char], mut body: impl FnMut(&FontFace)) -> bool {
        let db = FontDb::default_with_system_fonts();
        if db.is_empty() {
            eprintln!("skipping: no system fonts found");
            return false;
        }
        for i in 0..db.len() {
            let face = db
                .face(FaceId(i as u32))
                .expect("FaceId in 0..len must resolve");
            if required.iter().all(|c| face.glyph_index(*c).is_some()) {
                body(face);
                return true;
            }
        }
        eprintln!("skipping: no system face covered every required char: {required:?}");
        false
    }

    #[test]
    fn positioned_glyph_value_object() {
        // Pin the pub-fields shape — a refactor that hides x/y behind
        // accessors breaks the (intended) destructuring at call sites.
        let g = PositionedGlyph {
            glyph_id: GlyphId(65),
            x: 4.5,
            y: 0.0,
        };
        assert_eq!(g.glyph_id, GlyphId(65));
        assert_eq!(g.x, 4.5);
        assert_eq!(g.y, 0.0);
        let copy = g;
        assert_eq!(copy, g, "PositionedGlyph is Copy + PartialEq");
    }

    #[test]
    fn empty_string_returns_empty_vec() {
        // No font cover required — empty input is the canonical no-op.
        with_face_covering(&[], |face| {
            assert_eq!(shape_run(face, "", 14), vec![]);
            assert_eq!(shape_run(face, "abc", 0), vec![]);
        });
    }

    #[test]
    fn shape_run_hello_positions_match_advance_sum() {
        // AC anchor: test on 'hello' against known-good metrics from
        // a test font. "Known good" = the same `hor_advance` calls a
        // hand-rolled shaper would make. We cross-check the shaper
        // against the face's raw API — not against hard-coded numbers
        // (system fonts vary worker-to-worker).
        with_face_covering(&['h', 'e', 'l', 'o'], |face| {
            let size_px = 14u32;
            let glyphs = shape_run(face, "hello", size_px);
            assert_eq!(glyphs.len(), 5, "expected 5 glyphs for 'hello'");

            let scale = size_px as f32 / face.units_per_em() as f32;
            let mut expected_x = 0.0f32;
            for (idx, (c, glyph)) in "hello".chars().zip(glyphs.iter()).enumerate() {
                let expected_glyph_id = face
                    .glyph_index(c)
                    .expect("hello chars must have glyphs in the test font");
                assert_eq!(
                    glyph.glyph_id, expected_glyph_id,
                    "glyph[{idx}] = {c:?} expected glyph_id={expected_glyph_id:?}, got {:?}",
                    glyph.glyph_id
                );
                assert!(
                    (glyph.x - expected_x).abs() < 1e-3,
                    "glyph[{idx}].x: expected {expected_x}, got {}",
                    glyph.x
                );
                assert_eq!(glyph.y, 0.0, "glyph[{idx}].y: expected 0.0 on first line");
                let advance = face
                    .hor_advance(expected_glyph_id)
                    .expect("Latin glyphs must have hor_advance");
                expected_x += advance as f32 * scale;
            }
        });
    }

    #[test]
    fn shape_run_newline_advances_y_by_line_height() {
        // AC anchor: line height = ascender - descender + line_gap.
        // After a '\n', x resets to 0 and y advances by line_height.
        with_face_covering(&['a', 'b'], |face| {
            let size_px = 14u32;
            let glyphs = shape_run(face, "a\nb", size_px);
            assert_eq!(glyphs.len(), 2);

            let scale = size_px as f32 / face.units_per_em() as f32;
            let expected_line_height =
                (face.ascender() as f32 - face.descender() as f32 + face.line_gap() as f32) * scale;

            // First glyph: x=0, y=0.
            assert_eq!(glyphs[0].x, 0.0, "first glyph x must start at 0");
            assert_eq!(glyphs[0].y, 0.0, "first glyph y must start at 0");

            // Second glyph: x reset to 0, y advanced by line_height.
            assert_eq!(
                glyphs[1].x, 0.0,
                "post-newline glyph x must reset to 0, got {}",
                glyphs[1].x
            );
            assert!(
                (glyphs[1].y - expected_line_height).abs() < 1e-3,
                "post-newline glyph y: expected {expected_line_height}, got {}",
                glyphs[1].y
            );
        });
    }

    #[test]
    fn shape_run_drops_missing_codepoints() {
        // U+0000 (NULL) maps to no glyph in essentially every font.
        // We call shape_run with 'a' + U+0000 + 'b' and assert the
        // NULL is silently dropped, and that 'b' lands at advance('a')
        // (proving the dropped char contributed no advance).
        with_face_covering(&['a', 'b'], |face| {
            if face.glyph_index('\u{0000}').is_some() {
                eprintln!("skipping: face exposes glyph for U+0000");
                return;
            }
            let glyphs = shape_run(face, "a\u{0000}b", 14);
            assert_eq!(glyphs.len(), 2);
            let scale = 14.0 / face.units_per_em() as f32;
            let advance_a =
                face.hor_advance(face.glyph_index('a').unwrap()).unwrap() as f32 * scale;
            assert_eq!(glyphs[0].x, 0.0);
            assert!(
                (glyphs[1].x - advance_a).abs() < 1e-3,
                "'b' x: expected advance('a')={advance_a}, got {}",
                glyphs[1].x
            );
        });
    }
}
