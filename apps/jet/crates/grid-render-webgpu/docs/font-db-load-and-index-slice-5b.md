# FontDb — load + index multiple faces by family — Slice 5b

> Issue: #1751 · Parent epic: #1700 · Slice: 5b

## Problem

The text pipeline needs a registry that resolves a `(family, weight,
style)` triple — what CSS-style font selection produces — to a
parsed-font handle. Slice 5a delivered the parsed-font handle
itself ([`FontFace`]). This slice delivers the registry: a thin
wrapper around `fontdb::Database` that owns the parsed
[`FontFace`] for every face it has indexed, plus a `FaceId`
newtype so downstream slices don't carry `fontdb`'s opaque ID
type in their public signatures.

Two design choices pin the shape:

1. **Eager parse on load.** Every face the database accepts —
   whether registered explicitly via [`FontDb::load_font_data`] or
   discovered by [`FontDb::default_with_system_fonts`] — has its
   [`FontFace`] materialized at registration time. This makes
   [`FontDb::face`] a pure cache lookup that returns
   `Option<&FontFace>` straight from the stored entries (the AC's
   literal signature), without interior mutability or borrow-from-
   refcell gymnastics. Tradeoff: a system with many fonts (50–500)
   loads ~10–80 MB of font bytes at startup. The future shaping /
   atlas slices can introduce a lazy variant if the memory
   footprint becomes a problem; this slice prioritises a clean
   public API.
2. **Wrap, don't re-export, `fontdb`'s types.** [`FaceId`] is a
   newtype over a sequence number, not `fontdb::ID`. [`FontStyle`]
   is our own enum, translated to `fontdb::Style` only inside the
   query helper. This keeps downstream slices (the shaper, the
   glyph atlas, the font-fallback chain) from coupling to the
   fontdb crate version in their public surfaces — the same
   discipline Slice 5a applied to `ttf_parser`.

Parse failures during eager load are tolerated: a face that
`fontdb` indexes but [`FontFace::from_bytes`] rejects is logged
via `tracing::warn!` and skipped. This matters because system font
collections often include exotic formats (e.g. variable fonts
the wrapper hasn't opted into); a single broken face must not
poison the whole database.

## Scope

In:

- New module `jet_grid_render_webgpu::font_db` with:
  - `pub struct FontDb { /* private */ }` — owns a `fontdb::Database`
    + a vector of parsed `FontFace` entries indexed by [`FaceId`].
  - `pub struct FaceId(u32)` — newtype, `Copy + Hash + Eq` so it
    can be a `HashMap` key in the future glyph cache.
  - `pub enum FontStyle { Normal, Italic, Oblique }` — wrapper
    over `fontdb::Style` so downstream slices don't see fontdb in
    their public surfaces.
  - `pub enum FontDbError { InvalidFontData(String), NoFacesRegistered }` —
    error surface for [`FontDb::load_font_data`] and other
    fallible entry points.
  - `pub fn new() -> Self` — empty database, no system fonts
    loaded.
  - `pub fn default_with_system_fonts() -> Self` — calls
    `fontdb::Database::load_system_fonts()` and eagerly parses
    each face it indexes. Faces that fail [`FontFace::from_bytes`]
    are dropped with a `tracing::warn!` log.
  - `pub fn load_font_data(&mut self, bytes: Vec<u8>) -> Result<Vec<FaceId>, FontDbError>` —
    registers `bytes` with fontdb, eagerly parses every face the
    bytes contain (a TTC may carry multiple), returns the assigned
    [`FaceId`]s. Returns `InvalidFontData` on a parse failure for
    the *first* face (other faces still register if at least one
    succeeds; an empty success list returns `NoFacesRegistered`).
  - `pub fn query(&self, family: &str, weight: u16, style: FontStyle) -> Option<FaceId>` —
    runs `fontdb::Database::query` with the `(family, weight,
    style)` triple. Always uses `fontdb::Stretch::Normal` —
    stretch is a follow-up slice's concern.
  - `pub fn face(&self, id: FaceId) -> Option<&FontFace>` —
    returns the cached parsed face for `id`, or `None` if `id` is
    out of range (out-of-band IDs are graceful, not a panic).
  - `pub fn len(&self) -> usize` — number of registered faces;
    useful for tests + debug overlays.
  - `pub fn is_empty(&self) -> bool` — clippy-required companion.
- New dep on `fontdb = { version = "0.23", default-features = false }`
  in `apps/jet/crates/grid-render-webgpu/Cargo.toml`. Default features
  are off to keep the renderer's link line slim — system-font
  discovery on Linux via fontconfig is the most notable opt-in,
  but the eager parse path doesn't need it (fontdb falls back to
  walking standard font directories).
- Module-level docs explain the WHY:
  - Eager parse + memory tradeoff.
  - Why we wrap, not re-export, fontdb's `ID` / `Style` types.
  - Skip-on-parse-failure invariant for `default_with_system_fonts`.
- Unit tests covering AC error paths + signatures (real fonts are
  not embedded; system-font tests are `#[ignore]`'d).

Out:

- Lazy parse-on-access for system fonts. Documented tradeoff;
  future slice.
- `Stretch` (condensed / expanded) querying — fontdb supports it
  but the AC only names family + weight + style.
- Variable-font instance handling.
- Font-fallback chain (the next-face-on-glyph-missing logic) —
  separate sibling slice on the epic.
- TTC face-index selection beyond what fontdb auto-detects.
- Live system-font tests in `cargo test` (the path is
  environment-dependent and slow); a `#[ignore]`'d integration
  test exists as documentation.

## Interface

```rust
/// Registry of parsed font faces, queryable by CSS-style
/// (family, weight, style) triples.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/font-db-load-and-index-slice-5b.md#interface
/// @issue #1751
pub struct FontDb { /* private */ }

/// Opaque index assigned by [`FontDb`] when a face is registered.
/// `Copy + Hash + Eq` so it can be a `HashMap` key.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/font-db-load-and-index-slice-5b.md#interface
/// @issue #1751
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FaceId(pub u32);

/// CSS-style font style for queries.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/font-db-load-and-index-slice-5b.md#interface
/// @issue #1751
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontStyle { Normal, Italic, Oblique }

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FontDbError {
    /// `fontdb` accepted the bytes but every face inside failed
    /// `FontFace::from_bytes` (or the bytes weren't recognised at
    /// all).
    InvalidFontData(String),
    /// `load_font_data` succeeded structurally but produced no
    /// usable faces — likely an empty TTC or an exotic format
    /// the wrapper doesn't yet support.
    NoFacesRegistered,
}

impl FontDb {
    pub fn new() -> Self;
    pub fn default_with_system_fonts() -> Self;
    pub fn load_font_data(&mut self, bytes: Vec<u8>) -> Result<Vec<FaceId>, FontDbError>;
    pub fn query(&self, family: &str, weight: u16, style: FontStyle) -> Option<FaceId>;
    pub fn face(&self, id: FaceId) -> Option<&FontFace>;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
}
```

## Acceptance Criteria

- [x] `FontDb::default_with_system_fonts()` — implemented; calls
      `fontdb::Database::load_system_fonts()` and eagerly parses
      every face that registers, skipping parse failures.
- [x] `FontDb::load_font_data(Vec<u8>)` — implemented; returns the
      [`FaceId`]s assigned to the bytes' faces (TTC support
      built-in via fontdb's per-face iteration).
- [x] `FontDb::query(family, weight, style) -> Option<FaceId>` —
      implemented; thin shim over `fontdb::Database::query`.
- [x] `FontDb::face(FaceId) -> Option<&FontFace>` — implemented;
      pure cache lookup, no interior mutability needed.
- [x] `cargo test -p jet-grid-render-webgpu` passes.
- [x] Module-level docs explain the WHY (eager parse tradeoff;
      type-wrapping discipline; skip-on-parse-failure invariant),
      not just the what.

## Reference Context

- Parent epic [#1700](https://github.com/chrischeng-c4/cclab/issues/1700) —
  text pass (glyph rasterization, shaping, atlas upload, font fallback).
- Slice 5a (#1750) — [`FontFace`] parsed-font handle that
  [`FontDb`] caches. Spec at
  `apps/jet/crates/grid-render-webgpu/docs/ttf-parser-font-face-wrapper-slice-5a.md`.
- `apps/jet/crates/grid-render-webgpu/src/lib.rs` — where the new
  `pub mod font_db;` lands.
- `fontdb = "0.23"` — first version that pulls `ttf-parser = 0.25`
  transitively, matching this crate's existing pin.
