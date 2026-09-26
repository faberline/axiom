//! Glyph cache — Slice 5d (#1753).
//!
//! Stashes the output of [`crate::glyph_raster::rasterize_glyph`]
//! keyed by `(face_id, glyph_id, size_px)`, so the renderer doesn't
//! re-rasterize the same glyph on every frame and the atlas upload
//! (Slice 5f) has a canonical map to read from. One operation
//! matters — [`GlyphCache::insert_with_bitmap`] — and it is
//! **idempotent**: a second insert with the same key drops the new
//! raster and returns the cached entry. Upload paths rely on this:
//! they can `insert_with_bitmap` unconditionally without checking
//! membership first.
//!
//! Why bitmap-in-the-entry, not bitmap-out-of-band: the AC says
//! "Adds bitmap field to GlyphEntry behind a feature flag-free
//! design" — no Cargo feature, no `Option`, no separate buffer
//! pool. Simplicity now; if a future slice wants to free the
//! bitmap after upload it can drain via a follow-up
//! `release_bitmap(key)`. Premature to design that here.
//!
//! Why [`GlyphCache::insert_with_bitmap`] returns `&GlyphEntry`,
//! not the bitmap by move: the atlas upload reads `bitmap` +
//! `placement` simultaneously to compute atlas coordinates +
//! glyph-quad UVs. Returning `&GlyphEntry` binds both behind one
//! borrow, no split-borrow gymnastics at the call site.
//!
//! Cache is **append-only and unbounded** in this slice. LRU
//! eviction is a sibling concern; for a fixed UI font set (the only
//! consumer at this layer of the epic) the working set is small
//! enough that "everything in memory forever" is the right default.
//! Eviction lands when text input or zoom comes online.

use std::collections::HashMap;

use crate::font_db::FaceId;
use crate::font_face::GlyphId;
use crate::glyph_raster::GlyphBitmap;

/// Hash key for [`GlyphCache`] — uniquely identifies a rasterized
/// glyph by `(face_id, glyph_id, size_px)`. Two glyphs with the same
/// key are interchangeable from the renderer's perspective.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/glyph-cache-insert-with-bitmap-slice-5d.md#interface
/// @issue #1753
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GlyphKey {
    pub face_id: FaceId,
    pub glyph_id: GlyphId,
    pub size_px: u32,
}

/// Placement metrics for a rasterized glyph — everything in
/// [`GlyphBitmap`] except the pixel buffer. The atlas allocator
/// reads `width` / `height` to size the texture region; the layout
/// pass reads `baseline_offset` / `advance` to position the quad.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/glyph-cache-insert-with-bitmap-slice-5d.md#interface
/// @issue #1753
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    pub width: u32,
    pub height: u32,
    pub baseline_offset: i32,
    pub advance: f32,
}

/// One cached glyph: placement metrics + the alpha bitmap the
/// atlas uploads to the GPU.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/glyph-cache-insert-with-bitmap-slice-5d.md#interface
/// @issue #1753
#[derive(Debug, Clone, PartialEq)]
pub struct GlyphEntry {
    pub placement: Placement,
    pub bitmap: Vec<u8>,
}

/// Append-only cache from [`GlyphKey`] to [`GlyphEntry`]. See module
/// docs for the WHY behind the idempotent-insert + bitmap-in-entry
/// + unbounded design.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/glyph-cache-insert-with-bitmap-slice-5d.md#interface
/// @issue #1753
#[derive(Debug, Clone, Default)]
pub struct GlyphCache {
    entries: HashMap<GlyphKey, GlyphEntry>,
}

impl GlyphCache {
    /// Empty cache.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/glyph-cache-insert-with-bitmap-slice-5d.md#interface
    /// @issue #1753
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a rasterized glyph under `key`. On a key miss, store
    /// the split `(placement, bitmap)` pair and return a reference
    /// to the new entry. On a key hit, **drop `raster`** and return
    /// the existing entry — this is the idempotent contract the
    /// upload path relies on.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/glyph-cache-insert-with-bitmap-slice-5d.md#interface
    /// @issue #1753
    pub fn insert_with_bitmap(&mut self, key: GlyphKey, raster: GlyphBitmap) -> &GlyphEntry {
        self.entries.entry(key).or_insert_with(|| GlyphEntry {
            placement: Placement {
                width: raster.width,
                height: raster.height,
                baseline_offset: raster.baseline_offset,
                advance: raster.advance,
            },
            bitmap: raster.bitmap,
        })
    }

    /// Lookup — returns `None` if `key` was never inserted.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/glyph-cache-insert-with-bitmap-slice-5d.md#interface
    /// @issue #1753
    pub fn get(&self, key: GlyphKey) -> Option<&GlyphEntry> {
        self.entries.get(&key)
    }

    /// Number of cached entries.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/glyph-cache-insert-with-bitmap-slice-5d.md#interface
    /// @issue #1753
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// `true` when no entries are cached.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/glyph-cache-insert-with-bitmap-slice-5d.md#interface
    /// @issue #1753
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn key(face: u32, glyph: u16, size: u32) -> GlyphKey {
        GlyphKey {
            face_id: FaceId(face),
            glyph_id: GlyphId(glyph),
            size_px: size,
        }
    }

    fn raster(width: u32, height: u32, advance: f32) -> GlyphBitmap {
        GlyphBitmap {
            bitmap: vec![0u8; (width * height) as usize],
            width,
            height,
            baseline_offset: height as i32,
            advance,
        }
    }

    #[test]
    fn new_is_empty() {
        let c = GlyphCache::new();
        assert_eq!(c.len(), 0);
        assert!(c.is_empty());
        assert!(c.get(key(0, 0, 14)).is_none());
    }

    #[test]
    fn default_matches_new() {
        let c = GlyphCache::default();
        assert!(c.is_empty());
    }

    #[test]
    fn insert_returns_entry_on_miss() {
        let mut c = GlyphCache::new();
        let k = key(1, 65, 14); // 'A'
        let entry = c.insert_with_bitmap(k, raster(8, 10, 9.5));
        assert_eq!(entry.placement.width, 8);
        assert_eq!(entry.placement.height, 10);
        assert_eq!(entry.placement.advance, 9.5);
        assert_eq!(entry.bitmap.len(), 80);
        // Re-borrow via `get` to confirm storage.
        assert!(c.get(k).is_some());
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn insert_is_idempotent() {
        // AC anchor: idempotent insert returns same Placement.
        // The upload path calls `insert_with_bitmap` unconditionally;
        // this test pins the "second raster is dropped" contract.
        let mut c = GlyphCache::new();
        let k = key(1, 65, 14);
        let first_placement = c.insert_with_bitmap(k, raster(8, 10, 9.5)).placement;
        // Second insert with a deliberately different raster shape —
        // the cache must NOT replace the cached entry.
        let second_placement = c.insert_with_bitmap(k, raster(99, 99, 999.0)).placement;
        assert_eq!(
            first_placement, second_placement,
            "second insert must return the cached placement, not the new one"
        );
        assert_eq!(c.len(), 1, "cache size must not grow on idempotent insert");
    }

    #[test]
    fn distinct_keys_create_distinct_entries() {
        // Pin the key discrimination — different face / glyph /
        // size each produce a fresh entry.
        let mut c = GlyphCache::new();
        c.insert_with_bitmap(key(1, 65, 14), raster(8, 10, 9.5));
        c.insert_with_bitmap(key(2, 65, 14), raster(7, 11, 9.0)); // face differs
        c.insert_with_bitmap(key(1, 66, 14), raster(6, 10, 8.5)); // glyph differs
        c.insert_with_bitmap(key(1, 65, 18), raster(11, 14, 12.0)); // size differs
        assert_eq!(c.len(), 4);
    }

    #[test]
    fn get_returns_none_on_miss() {
        let c = GlyphCache::new();
        assert!(c.get(key(123, 456, 14)).is_none());
    }

    #[test]
    fn len_and_is_empty_track_inserts() {
        let mut c = GlyphCache::new();
        assert!(c.is_empty());
        c.insert_with_bitmap(key(0, 0, 14), raster(4, 4, 4.0));
        assert!(!c.is_empty());
        assert_eq!(c.len(), 1);
        // Idempotent — len must not change.
        c.insert_with_bitmap(key(0, 0, 14), raster(4, 4, 4.0));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn glyph_key_is_hash_eq_safe() {
        // Pin the `Hash + Eq` contract so a refactor that adds a
        // private field (e.g. a `_phantom: PhantomData`) fails here,
        // not in production.
        let mut by_key: HashMap<GlyphKey, &'static str> = HashMap::new();
        by_key.insert(key(1, 65, 14), "first");
        by_key.insert(key(1, 65, 14), "second"); // same key, replaces.
        by_key.insert(key(1, 65, 18), "third"); // size differs.
        assert_eq!(by_key.len(), 2);
        assert_eq!(by_key.get(&key(1, 65, 14)), Some(&"second"));
    }

    #[test]
    fn placement_value_object() {
        // If a refactor hides a Placement field behind an accessor,
        // the atlas slice's destructuring at the consumer crate
        // breaks first — surfacing here gives the refactor a single
        // anchor point in tests.
        let p = Placement {
            width: 4,
            height: 3,
            baseline_offset: 10,
            advance: 7.5,
        };
        assert_eq!(p.width, 4);
        assert_eq!(p.height, 3);
        assert_eq!(p.baseline_offset, 10);
        assert_eq!(p.advance, 7.5);
    }
}
