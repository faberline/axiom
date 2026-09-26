//! Font face registry — Slice 5b (#1751).
//!
//! Why this module exists: Slice 5a delivered [`FontFace`], the parsed
//! handle for a single font file. The text pipeline needs one more
//! layer on top — a registry that resolves a CSS-style `(family,
//! weight, style)` triple to a [`FontFace`]. That's what
//! [`FontDb`] is: a thin wrapper around `fontdb::Database` that owns
//! the parsed [`FontFace`] for every face it has indexed, plus a
//! [`FaceId`] newtype so downstream slices (shaper, glyph atlas,
//! fallback chain) don't carry `fontdb`'s opaque `ID` type in their
//! public signatures.
//!
//! Two design choices pin the shape, both documented because they
//! constrain future slices:
//!
//! 1. **Eager parse on load.** Every face that `fontdb` accepts gets
//!    its [`FontFace`] materialized at registration time and stored
//!    inside [`FontDb`]. This makes [`FontDb::face`] a pure cache
//!    lookup that returns `Option<&FontFace>` straight from the
//!    stored vec — the AC's literal signature — without interior
//!    mutability or `Ref<'_, _>` gymnastics. The tradeoff: a system
//!    with many fonts (50–500) loads ~10–80 MB of font bytes at
//!    startup. A future slice can introduce a lazy variant if the
//!    memory footprint becomes a problem; this slice prioritises a
//!    clean public API.
//!
//! 2. **Wrap, don't re-export, `fontdb`'s types.** [`FaceId`] is a
//!    newtype over a sequence number, not `fontdb::ID`. [`FontStyle`]
//!    is our own enum, translated to `fontdb::Style` only inside the
//!    query helper. This keeps downstream slices from coupling to
//!    the fontdb crate version in their public surfaces — the same
//!    discipline Slice 5a applied to `ttf_parser`.
//!
//! Skip-on-parse-failure invariant: a face that `fontdb` indexes but
//! [`FontFace::from_bytes`] rejects is logged via `tracing::warn!`
//! and dropped. System font collections often include exotic formats
//! (e.g. variable fonts the wrapper hasn't opted into); a single
//! broken face must not poison the whole database.
//!
//! Out of scope (sibling slices):
//! - Lazy parse-on-access for system fonts.
//! - `Stretch` (condensed / expanded) querying — fontdb supports it
//!   but the AC names only family + weight + style.
//! - Variable-font instance handling.
//! - Font-fallback chain (next-face-on-glyph-missing logic).
//! - TTC face-index selection beyond what fontdb auto-detects.

use std::collections::HashMap;
use std::fmt;

use crate::font_face::FontFace;

/// Opaque index assigned by [`FontDb`] when a face is registered.
/// `Copy + Hash + Eq` so it can be a `HashMap` key in the future
/// glyph cache.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/font-db-load-and-index-slice-5b.md#interface
/// @issue #1751
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FaceId(pub u32);

/// CSS-style font style for queries. Wraps `fontdb::Style` so
/// downstream slices don't see `fontdb` in their public surfaces.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/font-db-load-and-index-slice-5b.md#interface
/// @issue #1751
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontStyle {
    Normal,
    Italic,
    Oblique,
}

impl From<FontStyle> for fontdb::Style {
    fn from(style: FontStyle) -> Self {
        match style {
            FontStyle::Normal => fontdb::Style::Normal,
            FontStyle::Italic => fontdb::Style::Italic,
            FontStyle::Oblique => fontdb::Style::Oblique,
        }
    }
}

/// Errors returned by [`FontDb::load_font_data`].
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/font-db-load-and-index-slice-5b.md#interface
/// @issue #1751
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FontDbError {
    /// `fontdb` did not recognize the bytes as any known font format.
    /// No `FaceInfo` entries were produced from the buffer.
    InvalidFontData(String),
    /// `fontdb` accepted the bytes and produced one or more
    /// `FaceInfo` entries, but every face inside failed
    /// [`FontFace::from_bytes`] (e.g. exotic format the wrapper
    /// doesn't yet support). The bytes are still registered with
    /// `fontdb` but produced no usable [`FaceId`]s.
    NoFacesRegistered,
}

impl fmt::Display for FontDbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFontData(msg) => write!(f, "InvalidFontData: {msg}"),
            Self::NoFacesRegistered => write!(f, "NoFacesRegistered"),
        }
    }
}

impl std::error::Error for FontDbError {}

/// Registry of parsed font faces, queryable by CSS-style
/// `(family, weight, style)` triples. See module docs for the WHY
/// behind the eager-parse + type-wrapping design.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/font-db-load-and-index-slice-5b.md#interface
/// @issue #1751
pub struct FontDb {
    inner: fontdb::Database,
    faces: Vec<FontFace>,
    id_to_face_id: HashMap<fontdb::ID, FaceId>,
}

impl FontDb {
    /// Empty database — no system fonts loaded.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/font-db-load-and-index-slice-5b.md#interface
    /// @issue #1751
    pub fn new() -> Self {
        Self {
            inner: fontdb::Database::new(),
            faces: Vec::new(),
            id_to_face_id: HashMap::new(),
        }
    }

    /// Construct a database with the host's system fonts loaded.
    /// Every face that `fontdb` discovers is eagerly parsed; faces
    /// that fail [`FontFace::from_bytes`] are logged via
    /// `tracing::warn!` and skipped. See module docs for the
    /// skip-on-parse-failure invariant.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/font-db-load-and-index-slice-5b.md#interface
    /// @issue #1751
    pub fn default_with_system_fonts() -> Self {
        let mut db = Self::new();
        db.inner.load_system_fonts();
        let ids: Vec<fontdb::ID> = db.inner.faces().map(|info| info.id).collect();
        for id in ids {
            db.materialize_face(id);
        }
        db
    }

    /// Register `bytes` with the database, eagerly parsing every
    /// face the buffer contains (a TTC may carry multiple). Returns
    /// the [`FaceId`]s assigned to the successfully parsed faces.
    ///
    /// Errors:
    /// - [`FontDbError::InvalidFontData`] — `fontdb` did not
    ///   recognize the buffer as any known font format.
    /// - [`FontDbError::NoFacesRegistered`] — `fontdb` produced
    ///   `FaceInfo` entries but every face failed
    ///   [`FontFace::from_bytes`].
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/font-db-load-and-index-slice-5b.md#interface
    /// @issue #1751
    pub fn load_font_data(&mut self, bytes: Vec<u8>) -> Result<Vec<FaceId>, FontDbError> {
        let before: std::collections::HashSet<fontdb::ID> =
            self.inner.faces().map(|info| info.id).collect();
        self.inner.load_font_data(bytes);
        let new_ids: Vec<fontdb::ID> = self
            .inner
            .faces()
            .map(|info| info.id)
            .filter(|id| !before.contains(id))
            .collect();

        if new_ids.is_empty() {
            return Err(FontDbError::InvalidFontData(
                "fontdb did not recognize the buffer as any known font format".to_string(),
            ));
        }

        let mut assigned = Vec::with_capacity(new_ids.len());
        for id in new_ids {
            if let Some(face_id) = self.materialize_face(id) {
                assigned.push(face_id);
            }
        }

        if assigned.is_empty() {
            Err(FontDbError::NoFacesRegistered)
        } else {
            Ok(assigned)
        }
    }

    /// Resolve a CSS-style `(family, weight, style)` triple to a
    /// [`FaceId`]. Always uses `fontdb::Stretch::Normal` — stretch
    /// is a follow-up slice's concern.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/font-db-load-and-index-slice-5b.md#interface
    /// @issue #1751
    pub fn query(&self, family: &str, weight: u16, style: FontStyle) -> Option<FaceId> {
        let families = [fontdb::Family::Name(family)];
        let q = fontdb::Query {
            families: &families,
            weight: fontdb::Weight(weight),
            stretch: fontdb::Stretch::Normal,
            style: style.into(),
        };
        let id = self.inner.query(&q)?;
        self.id_to_face_id.get(&id).copied()
    }

    /// Look up the parsed face for `id`. Returns `None` if `id`
    /// is out of range — out-of-band IDs are graceful, not a panic.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/font-db-load-and-index-slice-5b.md#interface
    /// @issue #1751
    pub fn face(&self, id: FaceId) -> Option<&FontFace> {
        self.faces.get(id.0 as usize)
    }

    /// Number of successfully parsed and registered faces.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/font-db-load-and-index-slice-5b.md#interface
    /// @issue #1751
    pub fn len(&self) -> usize {
        self.faces.len()
    }

    /// `true` when no faces have been registered (clippy-required
    /// companion to [`Self::len`]).
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/font-db-load-and-index-slice-5b.md#interface
    /// @issue #1751
    pub fn is_empty(&self) -> bool {
        self.faces.is_empty()
    }

    /// Pull the font bytes for `id` from `fontdb`, parse them via
    /// [`FontFace::from_bytes`], and append to `self.faces` on
    /// success. Returns the assigned [`FaceId`] on success, `None`
    /// on parse failure (the failure is logged via `tracing::warn!`
    /// and the `fontdb` registration is left in place — only our
    /// parallel cache skips the entry).
    fn materialize_face(&mut self, id: fontdb::ID) -> Option<FaceId> {
        let parsed = self
            .inner
            .with_face_data(id, |data, _index| FontFace::from_bytes(data.to_vec()))?;
        match parsed {
            Ok(face) => {
                let face_id = FaceId(self.faces.len() as u32);
                self.faces.push(face);
                self.id_to_face_id.insert(id, face_id);
                Some(face_id)
            }
            Err(err) => {
                let info = self.inner.face(id);
                let family = info
                    .and_then(|i| i.families.first().map(|(n, _)| n.as_str()))
                    .unwrap_or("<unknown>");
                tracing::warn!(
                    "FontDb: dropping face {id:?} (family={family}): FontFace::from_bytes failed: {err}"
                );
                None
            }
        }
    }
}

impl Default for FontDb {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for FontDb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FontDb")
            .field("len", &self.faces.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn new_is_empty() {
        let db = FontDb::new();
        assert_eq!(db.len(), 0);
        assert!(db.is_empty());
    }

    #[test]
    fn default_is_empty() {
        // `Default::default` mirrors `new` — the system-font variant
        // is a separately named constructor on purpose.
        let db = FontDb::default();
        assert!(db.is_empty());
    }

    #[test]
    fn load_font_data_rejects_unrecognized_bytes() {
        // AC anchor: random bytes that fontdb can't decode produce
        // `InvalidFontData`. Distinguishable from `NoFacesRegistered`
        // (which is the "parsed by fontdb, rejected by FontFace"
        // path — much harder to provoke without a real font that
        // fontdb accepts but ttf-parser's wrapper rejects).
        let mut db = FontDb::new();
        let err = db
            .load_font_data(b"this is not a font".to_vec())
            .unwrap_err();
        assert!(
            matches!(err, FontDbError::InvalidFontData(_)),
            "expected InvalidFontData, got {err:?}"
        );
        // Failed load must not leak into the registry.
        assert!(db.is_empty());
    }

    #[test]
    fn query_returns_none_when_empty() {
        let db = FontDb::new();
        assert!(db.query("Helvetica", 400, FontStyle::Normal).is_none());
    }

    #[test]
    fn face_returns_none_for_unknown_id() {
        // Out-of-band IDs are graceful, not a panic. The shaper's
        // fallback chain depends on this — a stale FaceId from a
        // previous database revision must return `None`, not crash.
        let db = FontDb::new();
        assert!(db.face(FaceId(0)).is_none());
        assert!(db.face(FaceId(99_999)).is_none());
    }

    #[test]
    fn face_id_is_hash_eq_safe() {
        // FaceId will key the future glyph atlas's per-face cache.
        // Pin the `Hash + Eq` contract so a refactor that switches
        // FaceId to a tuple struct fails here, not in production.
        let mut by_face: HashMap<FaceId, &'static str> = HashMap::new();
        by_face.insert(FaceId(7), "first");
        by_face.insert(FaceId(7), "second"); // same key, replaces.
        by_face.insert(FaceId(8), "third");
        assert_eq!(by_face.len(), 2);
        assert_eq!(by_face.get(&FaceId(7)), Some(&"second"));
    }

    #[test]
    fn font_style_round_trips_to_fontdb() {
        // Pins the From<FontStyle> for fontdb::Style mapping so a
        // future variant addition (e.g. a non-CSS pseudo-style) is
        // caught by tests rather than silently mistranslated.
        assert!(matches!(
            fontdb::Style::from(FontStyle::Normal),
            fontdb::Style::Normal
        ));
        assert!(matches!(
            fontdb::Style::from(FontStyle::Italic),
            fontdb::Style::Italic
        ));
        assert!(matches!(
            fontdb::Style::from(FontStyle::Oblique),
            fontdb::Style::Oblique
        ));
    }

    #[test]
    fn font_db_error_display_invalid() {
        let e = FontDbError::InvalidFontData("garbage header".to_string());
        assert_eq!(e.to_string(), "InvalidFontData: garbage header");
    }

    #[test]
    fn font_db_error_display_no_faces() {
        let e = FontDbError::NoFacesRegistered;
        assert_eq!(e.to_string(), "NoFacesRegistered");
    }

    #[test]
    fn font_db_error_implements_std_error() {
        // Pins the `std::error::Error` impl so `?`-propagation
        // through `Box<dyn Error>` keeps working in callers.
        fn assert_error<T: std::error::Error>() {}
        assert_error::<FontDbError>();
    }

    /// Light smoke test: system-font loading completes without
    /// panic and `FontDb::face` returns a valid handle for every
    /// reported face. Marked `#[ignore]` because the path is
    /// environment-dependent (system font set varies wildly across
    /// hosts and CI) and slow (parses every face on disk). Run via
    /// `cargo test -p jet-grid-render-webgpu -- --ignored
    /// default_with_system_fonts_smoke`.
    #[test]
    #[ignore]
    fn default_with_system_fonts_smoke() {
        let db = FontDb::default_with_system_fonts();
        // Most desktop hosts have at least one usable face. If a
        // host happens to have zero, the assertion will document the
        // environmental gap rather than mask it.
        assert!(db.len() > 0, "expected at least one system font");
        for i in 0..db.len() {
            assert!(db.face(FaceId(i as u32)).is_some());
        }
    }
}
