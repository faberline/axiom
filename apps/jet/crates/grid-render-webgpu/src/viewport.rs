//! Viewport uniforms — the values bound at `@group(0) @binding(0)` for the
//! cell-rect pipeline.
//!
//! Why this module exists: the WGSL `Viewport` struct in
//! [`crate::cell_rect::CELL_RECT_WGSL`] and the Rust `ViewportUniforms`
//! struct here MUST agree on byte layout. Centralising the Rust definition
//! gives one place to look (and one place to keep in sync) when the shader
//! gains new uniforms.
//!
//! Layout invariant: WGSL `vec2<f32>` is 8-byte aligned at offset 0, and
//! WGSL rounds uniform buffers up to 16-byte alignment overall. The struct
//! is `size_px @ 0..8` + `scroll_px @ 8..16` — exactly 16 bytes, no
//! padding. The two `vec2<f32>`s satisfy WGSL alignment naturally; this
//! lets [`crate::WebGpuRenderer::on_scroll`] do an 8-byte partial write
//! at offset 8 without reasoning about pad bytes (Slice 4s, #1737).

use bytemuck::{Pod, Zeroable};

/// Byte offset of [`ViewportUniforms::scroll_px`] inside the
/// uniform-buffer payload. [`crate::WebGpuRenderer::on_scroll`] writes
/// 8 bytes here per scroll event; a partial write at this offset is the
/// minimum-payload contract Slice 4s (#1737) guarantees.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/scroll-driven-uniform-update-slice-4s.md#interface
/// @issue #1737
pub const SCROLL_PX_OFFSET_BYTES: wgpu::BufferAddress = 8;

/// Uniform values consumed by the cell-rect shader.
///
/// Layout matches the WGSL `Viewport` struct in
/// [`crate::cell_rect::CELL_RECT_WGSL`]: `size_px` is a `vec2<f32>` at
/// offset 0, `scroll_px` is a `vec2<f32>` at offset 8. Total 16 bytes —
/// the WGSL-required uniform alignment without any explicit padding.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/viewport-uniform-buffer-slice-4c.md#interface
/// @issue #1721
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct ViewportUniforms {
    pub size_px: [f32; 2],
    /// Physical-pixel offset of the visible window into the virtual
    /// sheet. The vertex shader subtracts this from each cell's
    /// virtual-sheet position to translate into the visible window
    /// (Slice 4s, #1737).
    pub scroll_px: [f32; 2],
}

impl ViewportUniforms {
    /// Construct a `ViewportUniforms` from physical pixel dimensions
    /// with `scroll_px = [0.0, 0.0]`.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/viewport-uniform-buffer-slice-4c.md#interface
    /// @issue #1721
    pub fn new(width_px: f32, height_px: f32) -> Self {
        Self {
            size_px: [width_px, height_px],
            scroll_px: [0.0; 2],
        }
    }

    /// Construct a `ViewportUniforms` carrying an existing scroll
    /// position alongside the new size. Used by internal renderer
    /// paths (resize, DPR change, device recovery) so a layout
    /// reflow preserves the user's scroll offset (Slice 4s, #1737).
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/scroll-driven-uniform-update-slice-4s.md#interface
    /// @issue #1737
    pub fn with_scroll(width_px: f32, height_px: f32, scroll_px: [f32; 2]) -> Self {
        Self {
            size_px: [width_px, height_px],
            scroll_px,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of;

    #[test]
    fn viewport_uniforms_is_16_bytes() {
        // AC invariant: WGSL uniform buffers must be 16-byte aligned. If a
        // future edit drops the padding the GPU silently reads garbage; this
        // test trips first.
        assert_eq!(size_of::<ViewportUniforms>(), 16);
    }

    #[test]
    fn size_px_lives_at_offset_zero() {
        // `Queue::write_buffer` writes raw bytes; if `size_px` ever moved
        // off offset 0 the shader would read the padding instead.
        let v = ViewportUniforms::new(1280.0, 720.0);
        let bytes: &[u8] = bytemuck::bytes_of(&v);
        let width: f32 = f32::from_le_bytes(bytes[0..4].try_into().unwrap());
        let height: f32 = f32::from_le_bytes(bytes[4..8].try_into().unwrap());
        assert_eq!(width, 1280.0);
        assert_eq!(height, 720.0);
    }

    #[test]
    fn new_zeros_scroll_px() {
        // The `_pad`-replacement field must default to [0, 0] under
        // the no-scroll constructor; the shader would otherwise pull
        // a garbage offset on the first frame (Slice 4s, #1737).
        let v = ViewportUniforms::new(1.0, 2.0);
        assert_eq!(v.scroll_px, [0.0; 2]);
    }

    #[test]
    fn with_scroll_lays_scroll_px_at_offset_8() {
        // `WebGpuRenderer::on_scroll` writes 8 bytes at offset 8 — the
        // partial-write contract relies on `scroll_px` living at the
        // documented offset (Slice 4s, #1737).
        let v = ViewportUniforms::with_scroll(1.0, 2.0, [10.0, -20.0]);
        let bytes: &[u8] = bytemuck::bytes_of(&v);
        let sx = f32::from_le_bytes(bytes[8..12].try_into().unwrap());
        let sy = f32::from_le_bytes(bytes[12..16].try_into().unwrap());
        assert_eq!(sx, 10.0);
        assert_eq!(sy, -20.0);
        // Sanity-check the documented offset constant matches.
        assert_eq!(SCROLL_PX_OFFSET_BYTES, 8);
    }
}
