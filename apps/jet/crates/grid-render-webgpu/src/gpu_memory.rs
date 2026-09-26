//! GPU memory accounting — approximate snapshot of the renderer's
//! allocated bytes. Slice 4bb (#1746).
//!
//! Why this module exists: two downstream consumers — devtools (Epic
//! 17) and the text-atlas eviction policy (Epic 13) — both need a
//! single byte number reflecting "how much GPU memory the grid
//! renderer is using right now". A precise allocator-hooked counter
//! would require instrumenting every `Device::create_*` call site
//! (textures, buffers, pipelines, query sets, …) and a thread-safe
//! shared atomic; that's invasive AND not what the consumers actually
//! need. Devtools wants order-of-magnitude accuracy for an overlay;
//! the eviction policy wants "is the renderer near budget".
//!
//! The model here is **derive-from-state**: at call time, the
//! renderer walks already-tracked fields (surface dims, format, MSAA
//! count, instance pool slot capacities, timing pool slot count) and
//! computes the report. Every allocation/drop mutates one of those
//! fields, so the next call to `gpu_memory_estimate` automatically
//! reflects it — no event hooks needed. This trades a small per-call
//! cost (a handful of multiplications + a vec scan) for never having
//! to keep a counter and the renderer state in sync.
//!
//! What's NOT counted: pipeline / shader-module / bind-group
//! state-object bytes (wgpu doesn't expose per-pipeline size, and
//! they're surface-independent constants the grid path doesn't churn
//! anyway), driver-side staging buffers, and the operating system's
//! window-system backbuffer rotation (the swap chain typically holds
//! 2–3 surface-sized textures, but the renderer only sees one
//! `surface_config`). The report is **conservative-low** but
//! captures the surface-scaled contributions that dominate as the
//! window grows.

use crate::WebGpuRenderer;

/// Approximate GPU memory footprint of this renderer's allocations.
/// All values are bytes; the report is a *snapshot* computed from
/// the renderer's tracked state, not a hooked-allocator counter.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/gpu-memory-accounting-slice-4bb.md#interface
/// @issue #1746
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GpuMemoryReport {
    /// Total bytes attributable to textures (surface backbuffer +
    /// MSAA color, when enabled).
    pub textures: u64,
    /// Total bytes attributable to buffers (uniforms, instance
    /// pool slots, timing-pool readback buffers).
    pub buffers: u64,
    /// Total bytes attributable to the glyph atlas. `0` until
    /// Slice 5a wires the text path.
    pub atlas: u64,
    /// Convenience sum: `textures + buffers + atlas`. Saturating
    /// `u64::MAX` on overflow (defensive — real GPU memory is
    /// bounded by VRAM long before this matters, but the sum is
    /// across user-controllable slot counts so an unbounded caller
    /// could theoretically overflow).
    pub total: u64,
}

impl GpuMemoryReport {
    /// Build a report from the three component totals and derive
    /// `total` via saturating addition.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/gpu-memory-accounting-slice-4bb.md#interface
    /// @issue #1746
    pub fn new(textures: u64, buffers: u64, atlas: u64) -> Self {
        let total = textures.saturating_add(buffers).saturating_add(atlas);
        Self {
            textures,
            buffers,
            atlas,
            total,
        }
    }
}

/// Per-pixel byte size for `format`. Wraps
/// [`wgpu::TextureFormat::block_copy_size`] with `None` (no aspect)
/// — returns `0` for compressed / depth / stencil formats that don't
/// have a well-defined per-pixel size. The grid path only allocates
/// `Rgba8Unorm` / `Bgra8Unorm` / `Bgra8UnormSrgb`, all of which
/// return `4`.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/gpu-memory-accounting-slice-4bb.md#interface
/// @issue #1746
pub fn bytes_per_pixel(format: wgpu::TextureFormat) -> u32 {
    format.block_copy_size(None).unwrap_or(0)
}

/// Bytes for a `width × height` texture of `format` — pure
/// `width * height * bytes_per_pixel(format)`. Callers can predict
/// the next allocation's footprint without going through the
/// renderer.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/gpu-memory-accounting-slice-4bb.md#interface
/// @issue #1746
pub fn texture_bytes(width: u32, height: u32, format: wgpu::TextureFormat) -> u64 {
    (width as u64)
        .saturating_mul(height as u64)
        .saturating_mul(bytes_per_pixel(format) as u64)
}

impl<'window> WebGpuRenderer<'window> {
    /// Snapshot the renderer's approximate GPU footprint. Cheap —
    /// reads tracked fields, runs no GPU command.
    ///
    /// The estimate is conservative: it covers the surface backbuffer,
    /// the MSAA color texture (when enabled), the viewport uniform,
    /// every instance-pool slot, and the timing pool's readback
    /// buffers. Pipeline state objects + driver staging are excluded
    /// — see the module-level doc for the full exclusion list.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/gpu-memory-accounting-slice-4bb.md#interface
    /// @issue #1746
    pub fn gpu_memory_estimate(&self) -> GpuMemoryReport {
        let surface_w = self.surface_config.width;
        let surface_h = self.surface_config.height;
        let format = self.surface_config.format;

        let backbuffer = texture_bytes(surface_w, surface_h, format);
        // MSAA color texture is `samples × backbuffer_bytes` — the
        // sample buffer is allocated only when `msaa_count > 1` AND
        // `msaa_color` is live.
        let msaa_bytes = if self.msaa_count > 1 && self.msaa_color.is_some() {
            backbuffer.saturating_mul(self.msaa_count as u64)
        } else {
            0
        };
        let textures = backbuffer.saturating_add(msaa_bytes);

        let viewport_uniform_bytes: u64 =
            std::mem::size_of::<crate::viewport::ViewportUniforms>() as u64;
        let instance_pool_bytes: u64 = (0..self.instance_pool.len())
            .map(|slot| self.instance_pool.capacity(slot) as u64)
            .sum();
        let timing_bytes = self.frame_timing.buffer_bytes_estimate();
        let buffers = viewport_uniform_bytes
            .saturating_add(instance_pool_bytes)
            .saturating_add(timing_bytes);

        // Atlas is reserved for Slice 5a+ (text glyph atlas). Until
        // then this field is structurally always `0`.
        let atlas = 0;

        GpuMemoryReport::new(textures, buffers, atlas)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texture_bytes_256x256_rgba8_is_256kib() {
        // Slice 4bb (#1746) AC: allocating a 256×256 RGBA8 texture
        // increases `textures` by 256 KiB. The estimate uses
        // `texture_bytes`; this anchors that formula at the value
        // the AC names.
        let bytes = texture_bytes(256, 256, wgpu::TextureFormat::Rgba8Unorm);
        assert_eq!(bytes, 256 * 1024, "256×256 RGBA8 must be exactly 256 KiB");
    }

    #[test]
    fn bytes_per_pixel_rgba8_is_4() {
        assert_eq!(bytes_per_pixel(wgpu::TextureFormat::Rgba8Unorm), 4);
        assert_eq!(bytes_per_pixel(wgpu::TextureFormat::Bgra8Unorm), 4);
        assert_eq!(bytes_per_pixel(wgpu::TextureFormat::Bgra8UnormSrgb), 4);
    }

    #[test]
    fn report_total_sums_components() {
        let r = GpuMemoryReport::new(1000, 200, 30);
        assert_eq!(r.textures, 1000);
        assert_eq!(r.buffers, 200);
        assert_eq!(r.atlas, 30);
        assert_eq!(r.total, 1230);
    }

    #[test]
    fn report_total_saturates_on_overflow() {
        // Defensive: a pathological caller summing near-u64::MAX
        // values must not panic on overflow.
        let r = GpuMemoryReport::new(u64::MAX, 1, 1);
        assert_eq!(r.total, u64::MAX, "saturating add must clamp to u64::MAX");
    }

    #[test]
    fn texture_bytes_zero_dim_is_zero() {
        // Zero-sized surface (minimized window — clamped to (1,1)
        // by the renderer constructor, but the pure fn must handle
        // it).
        assert_eq!(texture_bytes(0, 256, wgpu::TextureFormat::Rgba8Unorm), 0);
        assert_eq!(texture_bytes(256, 0, wgpu::TextureFormat::Rgba8Unorm), 0);
    }
}
