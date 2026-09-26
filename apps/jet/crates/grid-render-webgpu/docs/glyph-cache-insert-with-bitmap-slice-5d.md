# GlyphCache::insert_with_bitmap — Slice 5d

> Issue: #1753 · Parent epic: #1700 · Slice: 5d

## Problem

Slice 5c (#1752) delivered [`rasterize_glyph`] — turns a `(face,
glyph_id, size_px)` triple into a [`GlyphBitmap`] (alpha pixels +
placement metrics). The text pipeline now needs a place to **stash
that result**, keyed by the same triple, so the renderer doesn't
re-rasterize on every frame and the atlas upload (Slice 5f) has a
canonical map to read from.

This slice is `GlyphCache`: a `HashMap` keyed by
`GlyphKey { face_id, glyph_id, size_px }`, valued by a
`GlyphEntry { placement, bitmap }`. One operation matters —
`insert_with_bitmap(key, raster)` — and it's **idempotent**: a
second insert with the same key drops the new raster and returns
the cached entry. This is the contract the upload site relies on:
upload paths can call insert unconditionally without checking
membership first, and the cache promises not to duplicate work.

Two design choices pin the shape:

1. **Bitmap-in-the-cache, not bitmap-out-of-band.** `GlyphEntry`
   owns its `Vec<u8>` of alpha bytes directly. The AC says
   "Adds bitmap field to GlyphEntry behind a feature flag-free
   design" — no Cargo feature, no `Option`, no separate buffer
   pool. Simplicity now; if the atlas slice (5f) wants to free
   the bitmap after upload it can drain via a future
   `release_bitmap(key)`. Premature.
2. **`insert_with_bitmap` returns `&GlyphEntry`, not the bitmap by
   move.** The atlas upload reads `bitmap` + `placement` + the
   key's `size_px` to compute atlas coordinates — it needs both
   simultaneously. Returning `&GlyphEntry` lets the caller bind
   both with one borrow.

The cache is **append-only and unbounded** in this slice. LRU
eviction is a sibling concern; for a fixed UI font set (the only
consumer at this layer of the epic) the working set is small
enough that "everything in memory forever" is the right default.
Eviction lands when text input or zoom comes online.

## Scope

In:

- New module `jet_grid_render_webgpu::glyph_cache` with:
  - `pub struct GlyphKey { face_id: FaceId, glyph_id: GlyphId, size_px: u32 }`
    — `Copy + Hash + Eq` so it can key a `HashMap`. Three fields,
    no internal padding, hashed by tuple-of-fields default.
  - `pub struct Placement { width, height, baseline_offset, advance }`
    — copy of [`GlyphBitmap`]'s metrics without the `Vec<u8>`. The
    atlas slice needs to read placement during layout (before any
    bitmap touches the GPU); separating it from `bitmap` makes the
    inexpensive lookup cheap.
  - `pub struct GlyphEntry { placement: Placement, bitmap: Vec<u8> }`
    — value object, `pub` fields.
  - `pub struct GlyphCache { entries: HashMap<GlyphKey, GlyphEntry> }`
    — opaque; access via the methods below.
  - `pub fn new() -> Self` and `impl Default`.
  - `pub fn insert_with_bitmap(&mut self, key: GlyphKey, raster: GlyphBitmap) -> &GlyphEntry`
    — idempotent. On hit: drop `raster`, return the existing entry.
    On miss: split the `GlyphBitmap` into `(Placement, Vec<u8>)`,
    store, return.
  - `pub fn get(&self, key: GlyphKey) -> Option<&GlyphEntry>` —
    lookup-only.
  - `pub fn len(&self) -> usize` — entry count.
  - `pub fn is_empty(&self) -> bool` — clippy companion.
- Module-level docs explain the WHY:
  - Why bitmap lives in `GlyphEntry`, not a side buffer.
  - Why `insert_with_bitmap` returns `&GlyphEntry`.
  - Why unbounded/append-only is acceptable for this slice.
- Unit tests:
  - `insert_returns_entry_on_miss` — empty cache, insert, get the
    same key back.
  - `insert_is_idempotent` — insert twice with the same key, the
    second `raster` is dropped, returned entry equals the first
    one (`Placement` equality).
  - `get_returns_none_on_miss` — empty cache lookup.
  - `len_and_is_empty_track_inserts` — len() increments only on
    misses.
  - `glyph_key_is_hash_eq_safe` — pin the HashMap-key contract.
  - `placement_value_object` — pin the pub-fields shape.

Out:

- LRU / size-bounded eviction. Future slice when text input lands.
- Atlas-coordinate allocation (which rectangle in the texture).
  Slice 5e / 5f own that.
- Background pre-rasterization. The cache is filled lazily from
  the render path.
- Multi-thread access. The renderer is single-threaded; if the
  shaper ever runs off-thread, wrap the cache in a `Mutex` at the
  call site or revisit then.

## Interface

```rust
/// Hash key for [`GlyphCache`] — uniquely identifies a rasterized
/// glyph by `(face, glyph_id, size_px)`. Two glyphs with the same
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

/// Placement metrics for a rasterized glyph (everything in
/// [`GlyphBitmap`] except the pixel buffer). The atlas allocator
/// reads `width` / `height`; the layout pass reads
/// `baseline_offset` + `advance`.
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

/// One cached glyph — placement + the alpha bitmap the atlas
/// uploads to the GPU.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/glyph-cache-insert-with-bitmap-slice-5d.md#interface
/// @issue #1753
#[derive(Debug, Clone, PartialEq)]
pub struct GlyphEntry {
    pub placement: Placement,
    pub bitmap: Vec<u8>,
}

pub struct GlyphCache { /* private */ }

impl GlyphCache {
    pub fn new() -> Self;
    pub fn insert_with_bitmap(
        &mut self,
        key: GlyphKey,
        raster: GlyphBitmap,
    ) -> &GlyphEntry;
    pub fn get(&self, key: GlyphKey) -> Option<&GlyphEntry>;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
}
```

## Acceptance Criteria

- [x] `insert_with_bitmap(key, raster) -> &GlyphEntry` — implemented;
      idempotent on key collision.
- [x] Returns existing entry if cached; raster is dropped — handled
      via `HashMap::contains_key` + early return.
- [x] Adds bitmap field to `GlyphEntry` behind a feature flag-free
      design — `pub bitmap: Vec<u8>`, no Cargo feature, no `Option`.
- [x] Test: idempotent insert returns same `Placement` — covered
      by `insert_is_idempotent`.
- [x] `cargo test -p jet-grid-render-webgpu` passes.
- [x] Module-level docs explain the WHY (bitmap-in-entry,
      `&GlyphEntry` return shape, unbounded-cache rationale), not
      just the what.

## Reference Context

- Parent epic [#1700](https://github.com/chrischeng-c4/cclab/issues/1700) — text pass
  (glyph rasterization, shaping, atlas upload, font fallback).
- Slice 5a (#1750) — [`FontFace`] handle.
- Slice 5b (#1751) — [`FontDb`] registry, source of [`FaceId`].
- Slice 5c (#1752) — [`rasterize_glyph`], source of the
  [`GlyphBitmap`] that flows into `insert_with_bitmap`.
- Sibling slices 5e (#1754, R8 atlas descriptor), 5f (#1755, atlas
  upload via `queue.write_texture`) — downstream consumers that
  drain bitmaps from the cache to the GPU.
