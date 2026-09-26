//! TrueType / OpenType font face wrapper — Slice 5a (#1750).
//!
//! Why this module exists: `ttf_parser::Face<'a>` borrows from the
//! backing byte slice, so every owner up the call stack would have to
//! juggle that lifetime — viable for a one-off function, but viral
//! once the parsed face needs to flow through a glyph cache, a shaper,
//! and a font-fallback chain. [`FontFace`] hides that gymnastics
//! behind an owned `Vec<u8>` + a small struct of metrics cached on
//! construction.
//!
//! Why owned + lazy-reparse (not borrowed `Face`, not `owned-ttf-parser`):
//! a fresh `ttf_parser::Face::parse(&bytes, 0)` is microseconds — the
//! bytes already passed validation once at construction, and the
//! parser's work is just header offsets — so reconstructing for each
//! `glyph_index` / `hor_advance` call is cheap relative to the
//! shaping + rasterization that follows. Owning the bytes
//! (`Vec<u8>`) avoids a new crate dep (`owned-ttf-parser`
//! self-cells the equivalent for us, but adds an audit surface) and
//! matches the in-tree precedent at
//! [`jet_wasm::text::font_face::FontFace`] — same idiom, different
//! shaper. The metrics that never change (`units_per_em`,
//! `ascender`, `descender`, `line_gap`) are snapshotted once so the
//! common case doesn't reparse.
//!
//! Sign convention on [`FontFace::descender`]: we return the raw
//! `ttf_parser` value — typically negative for fonts whose
//! descender sits below the baseline in font-design units. Callers
//! that want a positive "descent" magnitude must `.abs()`; this
//! wrapper does not editorialize. Documented here so future
//! callers don't accidentally double-negate.
//!
//! Out of scope (sibling slices):
//! - File-path loading (one-liner `std::fs::read` + `from_bytes`).
//! - TTC face-index selection (Slice 5b's registry will need it).
//! - Variable-font instance handling.
//! - Vertical-writing metrics.
//! - Shaping (rustybuzz / harfbuzz_rs), rasterization
//!   (fontdue / swash), atlas upload.

use std::fmt;

/// Newtype over a 16-bit glyph identifier so downstream public
/// surfaces don't carry a direct `ttf_parser` type. Round-trips to
/// `ttf_parser::GlyphId` via [`From`] / [`Into`].
///
/// `Copy + Hash + Eq` so it can be a `HashMap` key in the future
/// glyph atlas slice.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/ttf-parser-font-face-wrapper-slice-5a.md#interface
/// @issue #1750
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GlyphId(pub u16);

impl From<ttf_parser::GlyphId> for GlyphId {
    fn from(value: ttf_parser::GlyphId) -> Self {
        Self(value.0)
    }
}

impl From<GlyphId> for ttf_parser::GlyphId {
    fn from(value: GlyphId) -> Self {
        ttf_parser::GlyphId(value.0)
    }
}

/// Errors returned by [`FontFace::from_bytes`].
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/ttf-parser-font-face-wrapper-slice-5a.md#interface
/// @issue #1750
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FontFaceError {
    /// Bytes are not a valid TrueType / OpenType font (or face
    /// index 0 isn't usable).
    InvalidData(String),
    /// A font header field falls outside the range the wrapper
    /// expects (e.g. `units_per_em == 0`, which would NaN the
    /// scaling math downstream).
    MetricsOutOfRange(String),
}

impl fmt::Display for FontFaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidData(msg) => write!(f, "InvalidData: {msg}"),
            Self::MetricsOutOfRange(msg) => write!(f, "MetricsOutOfRange: {msg}"),
        }
    }
}

impl std::error::Error for FontFaceError {}

/// Owned-bytes wrapper over `ttf_parser::Face`. See module docs for
/// the WHY behind the owned + lazy-reparse design.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/ttf-parser-font-face-wrapper-slice-5a.md#interface
/// @issue #1750
#[derive(Debug, Clone)]
pub struct FontFace {
    bytes: Vec<u8>,
    units_per_em: u16,
    ascender: i16,
    descender: i16,
    line_gap: i16,
}

impl FontFace {
    /// Parse a TrueType / OpenType font from owned bytes. The bytes
    /// are validated once (parser construction + metrics snapshot);
    /// subsequent glyph lookups re-parse a fresh `Face` on demand
    /// from the stored bytes.
    ///
    /// `Vec<u8>` (not `&[u8]`) per the AC — callers move ownership
    /// in so the wrapper has the only handle to the buffer.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/ttf-parser-font-face-wrapper-slice-5a.md#interface
    /// @issue #1750
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, FontFaceError> {
        let face = ttf_parser::Face::parse(&bytes, 0)
            .map_err(|e| FontFaceError::InvalidData(format!("ttf_parser::Face::parse: {e:?}")))?;

        let units_per_em = face.units_per_em();
        if units_per_em == 0 {
            return Err(FontFaceError::MetricsOutOfRange(
                "units_per_em is 0 — scaling math would divide by zero".to_string(),
            ));
        }

        let ascender = face.ascender();
        let descender = face.descender();
        let line_gap = face.line_gap();

        // Drop the borrowed `Face` before moving `bytes` into the
        // struct — `Face` borrows from `bytes`, so we cannot have
        // both live simultaneously.
        drop(face);

        Ok(Self {
            bytes,
            units_per_em,
            ascender,
            descender,
            line_gap,
        })
    }

    /// Cached `units_per_em` from the font header.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/ttf-parser-font-face-wrapper-slice-5a.md#interface
    /// @issue #1750
    pub fn units_per_em(&self) -> u16 {
        self.units_per_em
    }

    /// Cached `ascender` in font-design units.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/ttf-parser-font-face-wrapper-slice-5a.md#interface
    /// @issue #1750
    pub fn ascender(&self) -> i16 {
        self.ascender
    }

    /// Cached `descender` in font-design units. Typically NEGATIVE
    /// for fonts whose descender sits below the baseline. See the
    /// module-level sign-convention note.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/ttf-parser-font-face-wrapper-slice-5a.md#interface
    /// @issue #1750
    pub fn descender(&self) -> i16 {
        self.descender
    }

    /// Cached `line_gap` in font-design units. Typically `>= 0`,
    /// but the wrapper returns whatever the font reports.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/ttf-parser-font-face-wrapper-slice-5a.md#interface
    /// @issue #1750
    pub fn line_gap(&self) -> i16 {
        self.line_gap
    }

    /// Look up the glyph index for `c` via the font's cmap.
    /// Returns `None` if the codepoint isn't mapped — the font
    /// fallback chain (sibling slice) will then try the next face.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/ttf-parser-font-face-wrapper-slice-5a.md#interface
    /// @issue #1750
    pub fn glyph_index(&self, c: char) -> Option<GlyphId> {
        // `parse` succeeded once at construction; the bytes haven't
        // changed (we own them; no interior mutability), so the
        // re-parse cannot fail. `expect` documents that invariant.
        let face = ttf_parser::Face::parse(&self.bytes, 0)
            .expect("FontFace bytes were validated at construction");
        face.glyph_index(c).map(GlyphId::from)
    }

    /// Look up the horizontal advance for `glyph_id` via the font's
    /// hmtx table. Returns `None` if the glyph index is out of
    /// range (the font's `numGlyphs` boundary).
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/ttf-parser-font-face-wrapper-slice-5a.md#interface
    /// @issue #1750
    pub fn hor_advance(&self, glyph_id: GlyphId) -> Option<u16> {
        let face = ttf_parser::Face::parse(&self.bytes, 0)
            .expect("FontFace bytes were validated at construction");
        face.glyph_hor_advance(glyph_id.into())
    }

    /// Borrow the owned byte buffer. Escape hatch for slices that
    /// need to hand the same bytes to a different parser
    /// (`rustybuzz::Face::from_slice`, etc.) without an extra
    /// allocation.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/ttf-parser-font-face-wrapper-slice-5a.md#interface
    /// @issue #1750
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn from_bytes_rejects_invalid_data() {
        // AC anchor: random bytes return `InvalidData`. The
        // upstream font-fallback chain depends on this error
        // surface being distinguishable from "valid font, glyph
        // missing".
        let err = FontFace::from_bytes(b"this is not a font".to_vec()).unwrap_err();
        assert!(
            matches!(err, FontFaceError::InvalidData(_)),
            "expected InvalidData, got {err:?}"
        );
    }

    #[test]
    fn glyph_id_newtype_round_trips_with_ttf_parser() {
        // The newtype exists to keep ttf_parser out of the crate's
        // public surface for downstream slices. Round-trip
        // equality protects against accidental field reshuffles.
        for raw in [0u16, 1, 42, 0xFFFE] {
            let ours = GlyphId(raw);
            let ttf: ttf_parser::GlyphId = ours.into();
            let back: GlyphId = ttf.into();
            assert_eq!(ours, back);
            assert_eq!(ttf.0, raw);
        }
    }

    #[test]
    fn glyph_id_is_hash_eq_safe() {
        // The future glyph-atlas slice keys a HashMap on GlyphId.
        // This test pins the `Hash + Eq` contract so a refactor
        // that switches GlyphId to a tuple struct of (u16, u16)
        // (e.g. for face_index) fails here, not in production.
        let mut atlas: HashMap<GlyphId, &'static str> = HashMap::new();
        atlas.insert(GlyphId(42), "first");
        atlas.insert(GlyphId(42), "second"); // same key, replaces.
        atlas.insert(GlyphId(43), "third");
        assert_eq!(atlas.len(), 2);
        assert_eq!(atlas.get(&GlyphId(42)), Some(&"second"));
    }

    #[test]
    fn font_face_error_display_invalid_data() {
        let e = FontFaceError::InvalidData("bad header".to_string());
        assert_eq!(e.to_string(), "InvalidData: bad header");
    }

    #[test]
    fn font_face_error_display_metrics_out_of_range() {
        let e = FontFaceError::MetricsOutOfRange("units_per_em == 0".to_string());
        assert_eq!(e.to_string(), "MetricsOutOfRange: units_per_em == 0");
    }

    #[test]
    fn font_face_error_implements_std_error() {
        // Pins the `std::error::Error` impl so downstream
        // `?`-propagation through `Box<dyn Error>` keeps working.
        fn assert_error<T: std::error::Error>() {}
        assert_error::<FontFaceError>();
    }
}
