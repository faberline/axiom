# upload_glyph — Slice 5f

> Issue: #1755 · Parent epic: #1700 · Slice: 5f

## Problem

Slice 5d (#1753) delivered the CPU-side glyph cache; Slice 5e (#1754)
locked the GPU-side atlas descriptor. The missing piece is the **upload
glue**: given a position `(x, y)` in the atlas and a glyph bitmap of
size `(width, height)`, copy that bitmap into the atlas texture so the
text-pass bind group (Slice 5h) can sample it.

`wgpu::Queue::write_texture` is the right tool — it's the
"submit-and-forget" path that hides command-encoder bookkeeping for
small, one-shot writes. The glue is small but has three failure modes
that *must* be caught at upload time, not at sample time:

1. **Bitmap-size mismatch.** `bitmap.len()` must equal `width * height`
   bytes (one byte per pixel, since the atlas is `R8Unorm`). A
   too-short slice causes a silent partial write; a too-long slice
   silently truncates. Both produce subtly-wrong glyphs at sample time
   that look like cache corruption from the GPU side.
2. **Out-of-bounds placement.** `placement.x + width` must be `<=
   atlas_width`, and `placement.y + height` must be `<= atlas_height`.
   wgpu's validation layer catches this but emits a hard panic; we
   want a typed error the caller can recover from (e.g. evict + repack
   on overflow in a future allocator slice).
3. **Zero-size write.** `width == 0` or `height == 0` is a programming
   bug — the caller's allocator handed us a degenerate rect. Better to
   fail loud here than to let the no-op flow downstream confuse the
   atlas allocator's free-list bookkeeping.

The function takes the atlas texture by reference, reads `texture.width()` /
`texture.height()` from the live texture, and validates against those
runtime dimensions. The descriptor's `R8Unorm` + `D2` shape is implicit
in how we compute the row stride (`bytes_per_row = width * 1`); no
format negotiation needed.

## Scope

In:

- New module `jet_grid_render_webgpu::glyph_atlas_upload` with:
  - `pub struct AtlasPlacement { pub x: u32, pub y: u32 }` — pure value
    object naming the atlas-space origin of the glyph rect. Distinct
    from `glyph_cache::Placement` (which carries bitmap metrics, not
    atlas coordinates).
  - `pub enum AtlasUploadError { BitmapSizeMismatch { expected, actual }, OutOfBounds { x, y, width, height, atlas_width, atlas_height }, ZeroSize }`
    — typed errors so the caller can route recovery (retry vs. evict
    vs. panic).
  - `pub fn upload_glyph(queue: &wgpu::Queue, atlas: &wgpu::Texture, placement: AtlasPlacement, bitmap: &[u8], width: u32, height: u32) -> Result<(), AtlasUploadError>`
    — the AC entry point. Validates the three failure modes, then
    issues one `queue.write_texture` call.
- Module-level docs explain the WHY:
  - Why `queue.write_texture`, not encoder-based `copy_buffer_to_texture`
    (no readback, single small upload, hide command-encoder allocation).
  - Why validation is a typed `Result`, not `assert!` (callers want
    recovery; asserts kill the renderer thread).
  - Why `R8Unorm` makes `bytes_per_row = width * 1` (no padding for a
    one-byte format — the buffer-layout uses the natural tightly-packed
    row stride).
- Unit tests (no GPU required):
  - `upload_rejects_too_short_bitmap` — bitmap.len() < w*h → `BitmapSizeMismatch`.
  - `upload_rejects_too_long_bitmap` — bitmap.len() > w*h → `BitmapSizeMismatch`.
  - `upload_rejects_oob_x` — placement.x + width > atlas_width → `OutOfBounds`.
  - `upload_rejects_oob_y` — placement.y + height > atlas_height → `OutOfBounds`.
  - `upload_rejects_zero_width` / `upload_rejects_zero_height` → `ZeroSize`.
  - `error_display_covers_required_variants` — `Display` impl returns
    non-empty messages for every variant.
  - `placement_value_object` — pin the `pub x, y` shape.
- Integration test gated by `#[ignore]` (matches Slice 4y
  `screenshot_round_trip_known_pattern_live` pattern):
  - `upload_glyph_writes_pixels_live` — request a smoke adapter, build
    a 64×64 atlas via `glyph_atlas_texture_descriptor`, upload a 4×4
    bitmap of `0xFF` bytes at `(8, 8)`, copy the atlas back into a
    MAP_READ buffer (atlas needs `COPY_SRC` for the live test —
    handled inside the test via a parallel `device.create_texture`
    with `COPY_DST | COPY_SRC` since the production atlas doesn't
    surface `COPY_SRC`), assert the 4×4 rect at `(8, 8)` reads `0xFF`
    and a sample at `(0, 0)` reads `0x00`.

Out:

- Atlas allocator (which sub-rect each glyph lands in). The caller
  hands `upload_glyph` a placement; allocation policy is a sibling
  slice (5g territory).
- Eviction / repacking on overflow. Future slice once text input or
  zoom comes online.
- Async upload pipeline / coalesced multi-glyph writes. `write_texture`
  is the right primitive for the small-write case; a batched encoder
  path can come later if profiling demands.
- Multi-mip / multi-array upload. The atlas is `mip = 1`, `D2`,
  single-layer — those degrees of freedom don't exist.

## Interface

```rust
/// Atlas-space origin of a glyph rect. The rect spans
/// `[x, x + width) × [y, y + height)` in atlas pixel coordinates.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/glyph-atlas-upload-slice-5f.md#interface
/// @issue #1755
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AtlasPlacement {
    pub x: u32,
    pub y: u32,
}

/// Failure modes for [`upload_glyph`].
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/glyph-atlas-upload-slice-5f.md#interface
/// @issue #1755
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AtlasUploadError {
    BitmapSizeMismatch { expected: usize, actual: usize },
    OutOfBounds {
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        atlas_width: u32,
        atlas_height: u32,
    },
    ZeroSize,
}

/// Copy `bitmap` into `atlas` at `placement`, with the rect size
/// `(width, height)`. The atlas must have been created with
/// `R8Unorm` + `COPY_DST` (see [`crate::glyph_atlas`]). Returns
/// `Ok(())` on success or a typed error if validation fails before
/// the GPU is touched.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/glyph-atlas-upload-slice-5f.md#interface
/// @issue #1755
pub fn upload_glyph(
    queue: &wgpu::Queue,
    atlas: &wgpu::Texture,
    placement: AtlasPlacement,
    bitmap: &[u8],
    width: u32,
    height: u32,
) -> Result<(), AtlasUploadError>;
```

## Acceptance Criteria

- [x] `upload_glyph(queue, atlas, placement, bitmap, w, h)` —
      implemented; pure validation + one `queue.write_texture` call.
- [x] Uses `TexelCopyTextureInfo` (`ImageCopyTexture` pre-rename) +
      `TexelCopyBufferLayout` (`ImageDataLayout` pre-rename) — wgpu 24
      renamed both; the structural roles are unchanged.
- [x] Asserts placement + size fits within texture — typed
      `OutOfBounds` error, validated before the GPU call.
- [x] Test naga-validates the descriptor; queue path covered by
      integration test — descriptor created via Slice 5e's
      `glyph_atlas_texture_descriptor` (already test-pinned); queue
      path covered by `#[ignore]`-gated `upload_glyph_writes_pixels_live`.
- [x] `cargo test -p jet-grid-render-webgpu` passes.
- [x] Module-level docs explain the WHY (`write_texture` vs
      `copy_buffer_to_texture`, `Result` vs `assert!`, `bytes_per_row`
      derivation), not just the what.

## Reference Context

- Parent epic [#1700](https://github.com/chrischeng-c4/cclab/issues/1700) — text pass.
- Slice 5d (#1753) — `GlyphCache` produces the bitmaps + placement
  metrics that drive this upload.
- Slice 5e (#1754) — `glyph_atlas_texture_descriptor`. The R8Unorm +
  COPY_DST shape this upload writes through.
- Slice 5h (#1757) — text-pass bind group consumes the atlas this
  slice writes.
- `wgpu 24` rename: `ImageCopyTexture` → `TexelCopyTextureInfo`,
  `ImageDataLayout` → `TexelCopyBufferLayout`. The AC was authored
  against the older names; the implementation uses the new names. No
  semantic difference.
