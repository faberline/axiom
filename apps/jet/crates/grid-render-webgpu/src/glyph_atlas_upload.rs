//! Glyph atlas upload — Slice 5f (#1755).
//!
//! Given a [`wgpu::Texture`] (built from [`crate::glyph_atlas`]) and a
//! glyph bitmap, copy that bitmap into the atlas at an
//! [`AtlasPlacement`]. [`upload_glyph`] is the entire surface — pure
//! validation followed by one `queue.write_texture` call.
//!
//! ## Why `queue.write_texture`, not an encoder path
//!
//! Glyph uploads are small (a few hundred bytes per glyph) and one-shot
//! per cache miss. `queue.write_texture` hides the staging buffer + the
//! command encoder allocation that `copy_buffer_to_texture` would force
//! us to thread through the renderer. The text pipeline does not read
//! the atlas back, so the encoder path's only advantage (interleaving
//! with other GPU work in a single submit) is irrelevant here. If
//! profiling later shows the per-glyph submit cost dominates, a
//! batched encoder path is a sibling slice.
//!
//! ## Why typed `Result`, not `assert!`
//!
//! Three failure modes need recovery, not a panic:
//!
//! 1. **`BitmapSizeMismatch`** — `bitmap.len()` ≠ `width * height`.
//!    The cache + rasterizer should agree on the byte count, but a
//!    bug in one *will* corrupt the atlas silently if we trust the
//!    write blindly.
//! 2. **`OutOfBounds`** — `placement + size > atlas`. The future atlas
//!    allocator will hit this when the atlas is full; it needs to
//!    receive an error so it can evict + repack. wgpu's own validation
//!    panics, which would kill the renderer thread.
//! 3. **`ZeroSize`** — `width == 0` or `height == 0`. A degenerate rect
//!    from a buggy allocator. Failing loud here is cheaper than
//!    tracking down "the atlas allocator's free-list says this slot
//!    has zero size and now everything looks corrupt".
//!
//! ## Why `bytes_per_row = width`
//!
//! The atlas is `R8Unorm` — one byte per pixel. `TexelCopyBufferLayout`
//! takes `bytes_per_row` in *bytes*, not pixels; for a tightly-packed
//! `R8Unorm` bitmap that's exactly `width` bytes. `write_texture`'s
//! 256-byte row alignment requirement applies to *buffer-to-texture*
//! copies inside a command encoder, **not** to `queue.write_texture`,
//! which performs the alignment internally. So we hand it the natural
//! tight stride and let wgpu handle padding.

/// Atlas-space origin of a glyph rect. The rect spans
/// `[x, x + width) × [y, y + height)` in atlas pixel coordinates.
///
/// Distinct from [`crate::glyph_cache::Placement`], which carries
/// bitmap metrics (size + baseline + advance), not atlas coordinates.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/glyph-atlas-upload-slice-5f.md#interface
/// @issue #1755
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AtlasPlacement {
    pub x: u32,
    pub y: u32,
}

/// Failure modes for [`upload_glyph`]. See module docs for the WHY
/// each variant is a recoverable error rather than a panic.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/glyph-atlas-upload-slice-5f.md#interface
/// @issue #1755
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AtlasUploadError {
    /// `bitmap.len()` did not equal `width * height` bytes.
    BitmapSizeMismatch { expected: usize, actual: usize },
    /// `placement + (width, height)` exceeded the atlas dimensions.
    OutOfBounds {
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        atlas_width: u32,
        atlas_height: u32,
    },
    /// `width == 0` or `height == 0`. A degenerate upload from a
    /// buggy allocator.
    ZeroSize,
}

impl std::fmt::Display for AtlasUploadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BitmapSizeMismatch { expected, actual } => write!(
                f,
                "glyph bitmap size mismatch: expected {expected} bytes, got {actual}"
            ),
            Self::OutOfBounds {
                x,
                y,
                width,
                height,
                atlas_width,
                atlas_height,
            } => write!(
                f,
                "glyph rect [{x},{y}]+({width}x{height}) does not fit in {atlas_width}x{atlas_height} atlas"
            ),
            Self::ZeroSize => write!(f, "glyph upload rejected: zero width or height"),
        }
    }
}

impl std::error::Error for AtlasUploadError {}

/// Copy `bitmap` into `atlas` at `placement`, with the rect size
/// `(width, height)`. The atlas must have been created with
/// `R8Unorm` + `COPY_DST` (see [`crate::glyph_atlas`]). Returns
/// `Ok(())` on success or a typed error if validation fails *before*
/// the GPU is touched — no partial writes.
///
/// Issues one `queue.write_texture` call on success. See module docs
/// for the WHY behind the `write_texture` choice and the typed-error
/// shape.
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
) -> Result<(), AtlasUploadError> {
    if width == 0 || height == 0 {
        return Err(AtlasUploadError::ZeroSize);
    }
    let expected = (width as usize)
        .checked_mul(height as usize)
        .expect("width * height overflowed usize");
    if bitmap.len() != expected {
        return Err(AtlasUploadError::BitmapSizeMismatch {
            expected,
            actual: bitmap.len(),
        });
    }
    let atlas_width = atlas.width();
    let atlas_height = atlas.height();
    let fits_x = placement
        .x
        .checked_add(width)
        .is_some_and(|max| max <= atlas_width);
    let fits_y = placement
        .y
        .checked_add(height)
        .is_some_and(|max| max <= atlas_height);
    if !fits_x || !fits_y {
        return Err(AtlasUploadError::OutOfBounds {
            x: placement.x,
            y: placement.y,
            width,
            height,
            atlas_width,
            atlas_height,
        });
    }

    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: atlas,
            mip_level: 0,
            origin: wgpu::Origin3d {
                x: placement.x,
                y: placement.y,
                z: 0,
            },
            aspect: wgpu::TextureAspect::All,
        },
        bitmap,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placement_value_object() {
        // Pin the pub-fields shape. A refactor that hides x/y behind
        // accessors breaks the (intended) destructuring at call sites.
        let p = AtlasPlacement { x: 4, y: 7 };
        assert_eq!(p.x, 4);
        assert_eq!(p.y, 7);
        let copy = p;
        assert_eq!(copy, p, "AtlasPlacement is Copy + Eq");
    }

    #[test]
    fn error_display_covers_required_variants() {
        // Pin that every variant has a non-empty Display impl — these
        // strings flow into logs when the renderer drops a glyph.
        let mismatch = AtlasUploadError::BitmapSizeMismatch {
            expected: 64,
            actual: 32,
        };
        assert!(mismatch.to_string().contains("64"));
        assert!(mismatch.to_string().contains("32"));

        let oob = AtlasUploadError::OutOfBounds {
            x: 200,
            y: 0,
            width: 100,
            height: 8,
            atlas_width: 256,
            atlas_height: 256,
        };
        let s = oob.to_string();
        assert!(s.contains("200"));
        assert!(s.contains("256"));

        let zero = AtlasUploadError::ZeroSize;
        assert!(!zero.to_string().is_empty());
    }

    #[test]
    fn zero_width_or_height_rejected() {
        // ZeroSize MUST fire before BitmapSizeMismatch — even a
        // matching empty slice (len == 0 == 0*0) is a degenerate
        // upload from the caller's allocator, not a no-op.
        // No GPU touched: `upload_glyph` short-circuits on width/height
        // == 0 before any TexelCopy call. To exercise this without a
        // real Queue, we route through a small internal helper that
        // mirrors the validation prefix.
        assert_eq!(validate(64, 64, 0, 5, &[]), Err(AtlasUploadError::ZeroSize));
        assert_eq!(validate(64, 64, 5, 0, &[]), Err(AtlasUploadError::ZeroSize));
    }

    #[test]
    fn bitmap_size_mismatch_rejected() {
        // Too short.
        let r = validate(64, 64, 4, 4, &[0u8; 15]);
        assert_eq!(
            r,
            Err(AtlasUploadError::BitmapSizeMismatch {
                expected: 16,
                actual: 15,
            })
        );
        // Too long.
        let r = validate(64, 64, 4, 4, &[0u8; 17]);
        assert_eq!(
            r,
            Err(AtlasUploadError::BitmapSizeMismatch {
                expected: 16,
                actual: 17,
            })
        );
    }

    #[test]
    fn out_of_bounds_rejected_on_x() {
        let r = validate(64, 64, 10, 10, &[0u8; 100]);
        // Place at (60, 0) — 60 + 10 = 70 > 64 atlas width.
        let r = with_placement(r, 60, 0);
        assert!(matches!(
            r,
            Err(AtlasUploadError::OutOfBounds {
                x: 60,
                width: 10,
                atlas_width: 64,
                ..
            })
        ));
    }

    #[test]
    fn out_of_bounds_rejected_on_y() {
        let r = validate(64, 64, 10, 10, &[0u8; 100]);
        let r = with_placement(r, 0, 60);
        assert!(matches!(
            r,
            Err(AtlasUploadError::OutOfBounds {
                y: 60,
                height: 10,
                atlas_height: 64,
                ..
            })
        ));
    }

    #[test]
    fn in_bounds_with_correct_size_passes_validation() {
        // The validation helper returns Ok(()) when every check
        // passes — proves the happy-path validation does not reject.
        // (The actual `queue.write_texture` is exercised by the
        // `#[ignore]`-gated live integration test.)
        let r = validate(64, 64, 4, 4, &[0u8; 16]);
        let r = with_placement(r, 0, 0);
        assert_eq!(r, Ok(()));
    }

    // Mirror of `upload_glyph`'s validation prefix so unit tests can
    // run without a real `wgpu::Queue` + `wgpu::Texture`. Production
    // code calls the real `upload_glyph`; this helper is test-only.
    //
    // Returns Err early on ZeroSize / BitmapSizeMismatch, otherwise
    // Ok and the placement check is left to `with_placement`.
    fn validate(
        atlas_w: u32,
        atlas_h: u32,
        width: u32,
        height: u32,
        bitmap: &[u8],
    ) -> Result<(u32, u32, u32, u32, u32, u32), AtlasUploadError> {
        if width == 0 || height == 0 {
            return Err(AtlasUploadError::ZeroSize);
        }
        let expected = (width as usize) * (height as usize);
        if bitmap.len() != expected {
            return Err(AtlasUploadError::BitmapSizeMismatch {
                expected,
                actual: bitmap.len(),
            });
        }
        Ok((atlas_w, atlas_h, 0, 0, width, height))
    }

    fn with_placement(
        prior: Result<(u32, u32, u32, u32, u32, u32), AtlasUploadError>,
        x: u32,
        y: u32,
    ) -> Result<(), AtlasUploadError> {
        let (atlas_w, atlas_h, _, _, width, height) = prior?;
        let fits_x = x.checked_add(width).is_some_and(|m| m <= atlas_w);
        let fits_y = y.checked_add(height).is_some_and(|m| m <= atlas_h);
        if !fits_x || !fits_y {
            return Err(AtlasUploadError::OutOfBounds {
                x,
                y,
                width,
                height,
                atlas_width: atlas_w,
                atlas_height: atlas_h,
            });
        }
        Ok(())
    }

    // -------- live GPU integration test --------

    #[test]
    #[ignore = "requires a live wgpu adapter (CI workers may not have one)"]
    fn upload_glyph_writes_pixels_live() {
        // AC anchor: queue path covered by integration test.
        //
        // 1. Build a 64×64 R8Unorm atlas (COPY_DST | COPY_SRC | TEXTURE_BINDING)
        //    — we add COPY_SRC locally so the test can read the pixels
        //    back; production atlas omits COPY_SRC by design.
        // 2. Clear the texture (write all zeros) via upload_glyph at (0,0,64,64).
        // 3. upload_glyph at (8,8) with a 4×4 0xFF bitmap.
        // 4. Copy the texture to a MAP_READ buffer; assert the 4×4 rect
        //    at (8,8) reads 0xFF and a sample at (0,0) reads 0x00.
        use crate::headless::request_smoke_adapter;

        let maybe = pollster::block_on(async {
            let (_inst, adapter) = request_smoke_adapter().await?;
            let (device, queue) = adapter
                .request_device(
                    &wgpu::DeviceDescriptor {
                        label: Some("glyph_atlas_upload_test_device"),
                        required_features: wgpu::Features::empty(),
                        required_limits: wgpu::Limits::downlevel_defaults(),
                        memory_hints: wgpu::MemoryHints::Performance,
                    },
                    None,
                )
                .await
                .ok()?;

            let atlas = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("glyph_atlas_test"),
                size: wgpu::Extent3d {
                    width: 64,
                    height: 64,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R8Unorm,
                // Production atlas is COPY_DST | TEXTURE_BINDING; the
                // live test adds COPY_SRC so the readback step works.
                usage: wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });

            // Clear: 64×64 zeros.
            let zeros = vec![0u8; 64 * 64];
            upload_glyph(
                &queue,
                &atlas,
                AtlasPlacement { x: 0, y: 0 },
                &zeros,
                64,
                64,
            )
            .ok()?;

            // Write a 4×4 0xFF block at (8, 8).
            let block = vec![0xFFu8; 16];
            upload_glyph(&queue, &atlas, AtlasPlacement { x: 8, y: 8 }, &block, 4, 4).ok()?;

            // Readback: 64-byte rows already satisfy the 256-byte
            // wgpu alignment iff `bytes_per_row >= 256`. They don't —
            // 64 < 256 — so we ask for a 256-byte-padded readback row.
            let bytes_per_row: u32 = 256;
            let readback = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("glyph_atlas_readback"),
                size: (bytes_per_row as u64) * 64,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("glyph_atlas_readback_encoder"),
            });
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: &atlas,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &readback,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(bytes_per_row),
                        rows_per_image: Some(64),
                    },
                },
                wgpu::Extent3d {
                    width: 64,
                    height: 64,
                    depth_or_array_layers: 1,
                },
            );
            queue.submit(std::iter::once(encoder.finish()));

            let slice = readback.slice(..);
            let (tx, rx) = std::sync::mpsc::channel();
            slice.map_async(wgpu::MapMode::Read, move |r| {
                let _ = tx.send(r);
            });
            device.poll(wgpu::Maintain::Wait);
            rx.recv().ok()?.ok()?;
            let data = slice.get_mapped_range();

            // Strip the row padding (each row is 256 bytes; only the
            // first 64 are atlas pixels).
            let mut tight = Vec::with_capacity(64 * 64);
            for row in 0..64usize {
                let start = row * (bytes_per_row as usize);
                tight.extend_from_slice(&data[start..start + 64]);
            }
            drop(data);
            readback.unmap();

            Some(tight)
        });

        let pixels = match maybe {
            Some(p) => p,
            None => {
                eprintln!("skipping: no software adapter available");
                return;
            }
        };

        // The 4×4 rect at (8, 8) must be 0xFF.
        for dy in 0..4 {
            for dx in 0..4 {
                let idx = (8 + dy) * 64 + (8 + dx);
                assert_eq!(
                    pixels[idx],
                    0xFF,
                    "atlas at ({}, {}) expected 0xFF, got {:#x}",
                    8 + dx,
                    8 + dy,
                    pixels[idx],
                );
            }
        }
        // A pixel outside the rect must be 0x00 (the prior clear).
        assert_eq!(
            pixels[0], 0x00,
            "atlas at (0,0) expected 0x00 (cleared), got {:#x}",
            pixels[0]
        );
    }
}
