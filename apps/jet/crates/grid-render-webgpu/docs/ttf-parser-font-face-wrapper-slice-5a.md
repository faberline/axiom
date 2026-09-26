# ttf-parser font face wrapper — Slice 5a

> Issue: #1750 · Parent epic: #1700 · Slice: 5a

## Problem

The WebGPU text pipeline (epic #1700) needs a parsed-font handle that
the rasterizer + shaper + atlas layers can pass around. Working with
`ttf_parser::Face<'a>` directly forces every owner up the stack to
juggle the byte-slice lifetime — the parsed `Face` borrows from the
backing bytes, so the bytes must outlive any `Face`. That lifetime
gymnastics is fine for a single function, but viral once `Face` flows
through a glyph cache, a shaping context, and a fallback chain.

`FontFace` wraps the borrowed face behind an owned-bytes handle. The
crate has two viable options for "owned + lazy-borrow" in safe Rust:
the `owned-ttf-parser` crate (self-cell-based) or the
already-precedented pattern in `jet-wasm::text::font_face` —
store the bytes (cheap `Arc` or `Vec`) + cache the metrics on
construction, and re-parse a fresh `ttf_parser::Face` from the same
bytes on demand. The re-parse is cheap (microseconds; the bytes
already validated once at construction), no new crate dep, and
matches the existing in-tree idiom. We take that approach.

This slice is the **foundation layer** — only the metrics + glyph
lookup the text pipeline's other slices will call. Shaping
(rustybuzz / harfbuzz_rs), rasterization (fontdue / swash), and the
font registry (Slice 5b) are out of scope.

## Scope

In:

- New module `jet_grid_render_webgpu::font_face` with:
  - `pub struct FontFace { bytes: Vec<u8>, cached metrics }` —
    owns the byte buffer; cached metric fields snapshot
    `ttf_parser::Face` values that don't change.
  - `pub struct GlyphId(pub u16)` — newtype re-export of
    `ttf_parser::GlyphId`'s `.0` so downstream slices don't carry a
    direct `ttf_parser` type in their public signatures. Has
    `From<ttf_parser::GlyphId>` and `From<GlyphId> for ttf_parser::GlyphId`
    so internal call sites stay terse. `Debug + Clone + Copy + PartialEq + Eq + Hash`.
  - `pub enum FontFaceError { InvalidData(String), MetricsOutOfRange(String) }`
    with the standard `Display + Error` impls.
  - `pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, FontFaceError>` —
    validates by parsing once, caches the metrics, stores the bytes.
    `Vec<u8>` (not `&[u8]`) per the AC; callers move ownership in.
  - `pub fn units_per_em(&self) -> u16` — cached.
  - `pub fn ascender(&self) -> i16` — cached.
  - `pub fn descender(&self) -> i16` — cached. **Sign convention**:
    we return the ttf-parser raw value (typically negative — font
    design units below the baseline). Downstream callers that want
    a positive "descent" magnitude can `.abs()` — this slice does
    not editorialize.
  - `pub fn line_gap(&self) -> i16` — cached. Typically `>= 0` but
    we return whatever the font reports.
  - `pub fn glyph_index(&self, c: char) -> Option<GlyphId>` —
    constructs a fresh `Face` from the owned bytes and looks up
    the cmap.
  - `pub fn hor_advance(&self, glyph_id: GlyphId) -> Option<u16>` —
    constructs a fresh `Face` and reads the hmtx table.
  - `pub fn bytes(&self) -> &[u8]` — escape hatch for slices that
    need to hand the bytes to a different parser (rustybuzz, etc.).
- New dep on `ttf-parser = "0.25"` in
  `apps/jet/crates/grid-render-webgpu/Cargo.toml`. Same major as the
  in-tree `jet-wasm` consumer, so the workspace doesn't pull two
  copies.
- Module-level docs explaining the WHY:
  - Why owned + lazy-reparse instead of borrowed `Face`
    (lifetime virality; re-parse is microseconds).
  - Why not `owned-ttf-parser` (one fewer crate to audit; the
    in-tree precedent already uses the same lazy-reparse idiom).
  - Sign-convention note on `descender()` so future callers don't
    accidentally double-negate.
- Unit tests (host, no external fonts):
  - `from_bytes_rejects_invalid_data` — random bytes return
    `FontFaceError::InvalidData(_)`.
  - `glyph_id_newtype_round_trips_with_ttf_parser` — the
    `From`/`Into` between `font_face::GlyphId` and
    `ttf_parser::GlyphId` is byte-equivalent.
  - `glyph_id_is_hash_eq_safe` — placing two equal `GlyphId`s in a
    `HashMap<GlyphId, ()>` collapses to one entry (downstream
    glyph-atlas slices key on this).
  - `font_face_error_display_*` — the `Display` impls render the
    documented prefix.

Out:

- Loading from a file path. `from_path` is a one-liner over
  `std::fs::read` + `from_bytes`; not blocking and out of scope.
- TTC (TrueType Collection) face-index selection. The existing
  `jet-wasm` wrapper carries `face_index`; this slice's AC only
  names plain TTF/OTF so we hard-code face index 0. A follow-up
  slice will add `from_bytes_with_index` when the registry needs it.
- Variable-font instance / named-instance handling.
- Vertical writing metrics (`vertical_advance`, etc.).
- Shaping, rasterization, atlas upload — sibling slices.
- Live-font tests. The AC doesn't require them; CI's deterministic
  unit tests cover the invariants. If a future slice wants
  end-to-end coverage, it should add a tiny test font under
  `apps/jet/crates/grid-render-webgpu/tests/fixtures/` so the binary
  blob is reviewable.

## Interface

```rust
/// Opaque parsed-font handle. Owns the byte buffer; reparses a
/// `ttf_parser::Face` on demand for glyph lookups. Metrics that
/// never change (units_per_em, ascender, descender, line_gap) are
/// cached on construction so the hot path doesn't reparse for them.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/ttf-parser-font-face-wrapper-slice-5a.md#interface
/// @issue #1750
pub struct FontFace { /* private */ }

/// Newtype over a 16-bit glyph identifier so downstream public
/// surfaces don't carry a direct `ttf_parser` type. Round-trips
/// to `ttf_parser::GlyphId` via `From` / `Into`.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/ttf-parser-font-face-wrapper-slice-5a.md#interface
/// @issue #1750
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GlyphId(pub u16);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FontFaceError {
    /// Bytes are not a valid TrueType / OpenType font (or the
    /// face-index 0 entry isn't usable).
    InvalidData(String),
    /// A font header field falls outside the range the wrapper
    /// expects (e.g. `units_per_em == 0`, which would NaN the
    /// scaling math downstream).
    MetricsOutOfRange(String),
}

impl FontFace {
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, FontFaceError>;
    pub fn units_per_em(&self) -> u16;
    pub fn ascender(&self) -> i16;
    pub fn descender(&self) -> i16;
    pub fn line_gap(&self) -> i16;
    pub fn glyph_index(&self, c: char) -> Option<GlyphId>;
    pub fn hor_advance(&self, glyph_id: GlyphId) -> Option<u16>;
    pub fn bytes(&self) -> &[u8];
}
```

## Acceptance Criteria

- [x] `FontFace::from_bytes(Vec<u8>) -> Result<Self>` — implemented;
      `Vec<u8>` ownership transferred in, parsed once for validation,
      stored.
- [x] Owns the bytes via owned_ttf_parser style self-ref — implemented
      as owned `Vec<u8>` + lazy reparse on demand. Documented as a
      deliberate variant of the "owned + lazy-borrow" pattern.
- [x] `units_per_em()`, `ascender()`, `descender()`, `line_gap()` —
      cached on construction; pure accessors.
- [x] `glyph_index(char) -> Option<GlyphId>` — reparses + cmap lookup.
- [x] `hor_advance(GlyphId) -> Option<u16>` — reparses + hmtx lookup.
- [x] `cargo test -p jet-grid-render-webgpu` passes.
- [x] Module-level docs explain the WHY (owned + lazy-reparse vs
      borrowed `Face`; sign convention on `descender`; why not
      `owned-ttf-parser`), not just the what.

## Reference Context

- Parent epic [#1700](https://github.com/chrischeng-c4/cclab/issues/1700) — text pass
  (glyph rasterization, shaping, atlas upload, font fallback).
- `apps/jet/wasm/src/text/font_face.rs` — existing rustybuzz-based
  wrapper. Same "owned-bytes + lazy-reparse" idiom this slice adopts,
  but for the canvas-based shaping path; the WebGPU text pipeline
  needs its own ttf-parser-only wrapper since the renderer crate
  must not pull `rustybuzz`.
- `apps/jet/crates/grid-render-webgpu/src/lib.rs` — where the new
  `pub mod font_face;` lands.
- ttf-parser 0.25 — same major already pinned in
  `jet-wasm/Cargo.toml`; this slice reuses the version.
