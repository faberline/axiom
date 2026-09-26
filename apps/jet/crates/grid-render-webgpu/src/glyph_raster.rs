//! Single-glyph rasterizer — Slice 5c (#1752).
//!
//! Turns a [`FontFace`] + [`GlyphId`] + integer pixel size into an
//! 8-bit grayscale alpha bitmap plus the placement metrics the
//! caller needs to position the bitmap on the baseline. The
//! foundation primitive for the text pass's glyph atlas (sibling
//! slices 5d–5f).
//!
//! **Backend choice — fontdue, not swash / ab_glyph.** `fontdue::Font::
//! rasterize_indexed(glyph_id, px)` returns `(Metrics, Vec<u8>)` in
//! one call, with the exact bitmap shape the AC asks for — linear
//! grayscale, top-left origin, `len == width * height`. `swash` is
//! shaping + raster (huge surface for a foundation slice); `ab_glyph`
//! is similar in size to fontdue but requires us to translate
//! `OutlineBuilder` events to a bitmap by hand. fontdue is the
//! minimum viable backend.
//!
//! **`baseline_offset` sign convention** (the load-bearing one — wrong
//! sign here silently produces upside-down text): the value is the
//! distance from the **baseline** to the **top** edge of the bitmap,
//! in whole pixels, positive for glyphs that sit above the baseline
//! (the common case). Derived from fontdue's `(ymin + height)` —
//! fontdue's `ymin` is the bottom-edge offset, signed-negative when
//! the glyph descends below the baseline. `ymin + height` collapses
//! that into the more conventional "ascent of this glyph" magnitude
//! callers usually want at the layout site.
//!
//! **`advance` is subpixels, not whole pixels** — copied straight
//! from `Metrics::advance_width`. Layout code rounds when it commits
//! a baseline X position; rounding here would lose information for
//! tightly-spaced runs.
//!
//! **Re-parse on every call** is acceptable for this slice — fontdue's
//! parse is microseconds against `FontFace::bytes()` we already own.
//! Slice 5d (#1753) is where the `fontdue::Font` gets cached.
//!
//! Out of scope (sibling slices):
//! - Subpixel positioning (`rasterize_indexed_subpixel`) — AC says
//!   integer sizes for now.
//! - Glyph cache (Slice 5d / #1753).
//! - R8 atlas descriptor (Slice 5e / #1754) + atlas upload (5f / #1755).
//! - Shaping — caller hands us a pre-shaped [`GlyphId`].
//! - Color glyphs / SVG-in-OT / emoji — fontdue is monochrome only.

use std::fmt;

use fontdue::{Font, FontSettings};

use crate::font_face::{FontFace, GlyphId};

/// 8-bit grayscale alpha bitmap of a single rasterized glyph plus
/// the placement metrics needed to position it on a baseline. See
/// module docs for the sign convention on [`Self::baseline_offset`]
/// and the unit convention on [`Self::advance`].
///
/// All fields are `pub` because this is a value object — there are
/// no invariants beyond `bitmap.len() == (width * height) as usize`,
/// which the constructor enforces and consumers can destructure
/// freely.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/rasterize-glyph-bitmap-slice-5c.md#interface
/// @issue #1752
#[derive(Debug, Clone, PartialEq)]
pub struct GlyphBitmap {
    /// Linear grayscale alpha, top-left origin.
    /// `bitmap.len() == (width * height) as usize` for every
    /// successfully rasterized glyph (whitespace glyphs come back
    /// with `width == 0` / `height == 0` and an empty `bitmap`).
    pub bitmap: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// Distance from the baseline to the top edge of the bitmap,
    /// in whole pixels. Positive for glyphs above the baseline. See
    /// module docs for the WHY behind this specific sign convention.
    pub baseline_offset: i32,
    /// Horizontal advance in subpixels (fractional pixels).
    /// Layout code rounds when committing a baseline X position.
    pub advance: f32,
}

/// Errors returned by [`rasterize_glyph`].
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/rasterize-glyph-bitmap-slice-5c.md#interface
/// @issue #1752
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RasterError {
    /// `size_px == 0` — programmer bug. fontdue's scale factor
    /// would NaN on this; the explicit error surfaces it instead
    /// of letting `0 / 0` propagate into bitmap dimensions.
    ZeroSize,
    /// `glyph_id.0 >= font.glyph_count()` — a stale id from a
    /// previous font revision, or a caller that didn't constrain
    /// the id to the source font.
    GlyphOutOfRange(u16),
    /// `fontdue::Font::from_bytes` rejected the [`FontFace`]'s bytes.
    /// In principle unreachable for a `FontFace` that already
    /// passed Slice 5a's validation, but kept on the surface for
    /// the case where a future change introduces format
    /// divergence between ttf-parser and fontdue.
    FontParseFailed(String),
}

impl fmt::Display for RasterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroSize => write!(f, "ZeroSize"),
            Self::GlyphOutOfRange(id) => write!(f, "GlyphOutOfRange({id})"),
            Self::FontParseFailed(msg) => write!(f, "FontParseFailed: {msg}"),
        }
    }
}

impl std::error::Error for RasterError {}

/// Rasterize a single glyph from `face` at `size_px` whole pixels.
/// Returns a [`GlyphBitmap`] on success or a [`RasterError`] for
/// the three failure modes documented on the enum.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/rasterize-glyph-bitmap-slice-5c.md#interface
/// @issue #1752
pub fn rasterize_glyph(
    face: &FontFace,
    glyph_id: GlyphId,
    size_px: u32,
) -> Result<GlyphBitmap, RasterError> {
    if size_px == 0 {
        return Err(RasterError::ZeroSize);
    }

    let font = Font::from_bytes(face.bytes(), FontSettings::default())
        .map_err(|e| RasterError::FontParseFailed(e.to_string()))?;

    if glyph_id.0 >= font.glyph_count() {
        return Err(RasterError::GlyphOutOfRange(glyph_id.0));
    }

    let (metrics, bitmap) = font.rasterize_indexed(glyph_id.0, size_px as f32);

    // Whitespace glyphs come back with `width == 0` / `height == 0`
    // and an empty `bitmap`; the `usize -> u32` cast is safe because
    // fontdue can't produce a bitmap larger than `u32::MAX` (a 65535-
    // px-em-square is the TrueType upper bound).
    Ok(GlyphBitmap {
        bitmap,
        width: metrics.width as u32,
        height: metrics.height as u32,
        baseline_offset: metrics.ymin + metrics.height as i32,
        advance: metrics.advance_width,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font_db::{FaceId, FontDb};

    #[test]
    fn raster_error_display_zero_size() {
        assert_eq!(RasterError::ZeroSize.to_string(), "ZeroSize");
    }

    #[test]
    fn raster_error_display_out_of_range() {
        assert_eq!(
            RasterError::GlyphOutOfRange(42).to_string(),
            "GlyphOutOfRange(42)"
        );
    }

    #[test]
    fn raster_error_display_font_parse_failed() {
        let e = RasterError::FontParseFailed("bad header".to_string());
        assert_eq!(e.to_string(), "FontParseFailed: bad header");
    }

    #[test]
    fn raster_error_implements_std_error() {
        // Pins the `std::error::Error` impl so `?`-propagation
        // through `Box<dyn Error>` keeps working in callers.
        fn assert_error<T: std::error::Error>() {}
        assert_error::<RasterError>();
    }

    #[test]
    fn glyph_bitmap_value_object() {
        // Pin the public-field shape. If a refactor hides a field
        // behind an accessor, the upload site at the consumer crate
        // breaks first — surfacing here gives the refactor a single
        // anchor point in tests.
        let b = GlyphBitmap {
            bitmap: vec![0u8; 12],
            width: 4,
            height: 3,
            baseline_offset: 10,
            advance: 7.5,
        };
        assert_eq!(b.bitmap.len(), (b.width * b.height) as usize);
        assert_eq!(b.baseline_offset, 10);
        assert_eq!(b.advance, 7.5);
    }

    /// AC anchor: "Round-trips a known glyph (e.g. 'A' at 14 px)".
    ///
    /// No font fixture is vendored with the renderer (the
    /// `.aw/tech-design/` convention is to keep test assets out
    /// of the source tree where possible), so this test routes
    /// through [`FontDb::default_with_system_fonts`] and gracefully
    /// skips when the host has no usable fonts. macOS dev hosts
    /// always have system fonts; Linux CI typically has DejaVu /
    /// Liberation; headless minimal containers may not — those skip
    /// rather than fail.
    #[test]
    fn rasterize_a_glyph_round_trip() {
        let db = FontDb::default_with_system_fonts();
        if db.is_empty() {
            eprintln!("rasterize_a_glyph_round_trip: no system fonts found — skipping live raster");
            return;
        }
        // Walk every registered face looking for one that has a
        // glyph for 'A'. We don't pin a specific family because the
        // available set varies wildly across hosts.
        let mut tested = false;
        for i in 0..db.len() {
            let face = db
                .face(FaceId(i as u32))
                .expect("FaceId in 0..len must resolve");
            let Some(glyph_id) = face.glyph_index('A') else {
                continue;
            };
            let raster = rasterize_glyph(face, glyph_id, 14)
                .expect("'A' at 14 px must rasterize for a valid font");
            assert_eq!(
                raster.bitmap.len(),
                (raster.width * raster.height) as usize,
                "bitmap len must equal width * height"
            );
            assert!(raster.width > 0, "'A' must have non-zero width");
            assert!(raster.height > 0, "'A' must have non-zero height");
            assert!(raster.advance > 0.0, "'A' must have positive advance");
            tested = true;
            break;
        }
        if !tested {
            eprintln!(
                "rasterize_a_glyph_round_trip: no system face had a glyph for 'A' — skipping"
            );
        }
    }

    #[test]
    fn rasterize_zero_size_returns_zero_size_error() {
        // The function should reject `size_px == 0` before touching
        // fontdue. We don't have a FontFace handy without a system
        // font, but the error path is reachable on a stub face too —
        // we just need ANY FontFace. Skip if none available.
        let db = FontDb::default_with_system_fonts();
        let Some(face) = db.face(FaceId(0)) else {
            eprintln!("rasterize_zero_size_returns_zero_size_error: no system font, skipping");
            return;
        };
        let err = rasterize_glyph(face, GlyphId(0), 0).unwrap_err();
        assert_eq!(err, RasterError::ZeroSize);
    }

    #[test]
    fn rasterize_out_of_range_returns_out_of_range_error() {
        // Glyph id 65535 (u16::MAX) is past `numGlyphs` for every
        // real font.
        let db = FontDb::default_with_system_fonts();
        let Some(face) = db.face(FaceId(0)) else {
            eprintln!(
                "rasterize_out_of_range_returns_out_of_range_error: no system font, skipping"
            );
            return;
        };
        let err = rasterize_glyph(face, GlyphId(u16::MAX), 14).unwrap_err();
        assert!(
            matches!(err, RasterError::GlyphOutOfRange(_)),
            "expected GlyphOutOfRange, got {err:?}"
        );
    }
}
