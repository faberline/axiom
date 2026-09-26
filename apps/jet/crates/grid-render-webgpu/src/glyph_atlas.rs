//! Glyph atlas texture descriptor — Slice 5e (#1754).
//!
//! The text pass needs a single GPU texture that holds every rasterized
//! glyph as alpha pixels — the "glyph atlas". Slice 5d (#1753) delivered
//! the CPU-side [`crate::glyph_cache::GlyphCache`]; this slice locks the
//! **GPU-side texture shape** in one pure function so the upload (Slice
//! 5f), the bind group (Slice 5h), and the sampler (Slice 5i) all see the
//! same dimensions / format / usage flags.
//!
//! ## Why `R8Unorm`
//!
//! Slice 5c's [`crate::glyph_raster::rasterize_glyph`] produces 8-bit
//! grayscale alpha — a single channel per pixel. `R8Unorm` matches
//! one-for-one (`u8` → `[0.0, 1.0]` on shader read), is supported across
//! every wgpu backend the renderer targets (Metal, Vulkan, D3D12,
//! GL ES), and is 4× smaller in VRAM than `Rgba8`. Future color-glyph
//! slices can add a sibling `Rgba8UnormSrgb` atlas; that's not this slice.
//!
//! ## Why `COPY_DST | TEXTURE_BINDING`
//!
//! `COPY_DST` is what `queue.write_texture` writes through (Slice 5f
//! upload path); `TEXTURE_BINDING` is what the text-pass bind group
//! consumes (Slice 5h). No `RENDER_ATTACHMENT` — we don't draw INTO the
//! atlas. No `STORAGE_BINDING` — shader writes aren't part of this
//! pipeline. Two flags, exactly what the pipeline needs.
//!
//! ## Why `mip = 1`, `sample = 1`, `D2`
//!
//! Glyph rendering uses the bitmap at exactly one mip level (the atlas
//! slot's natural size); multisampling on an alpha lookup would burn
//! memory for no quality benefit; the third dimension is meaningless for
//! a 2D atlas.
//!
//! ## Why the paired sampler is `Linear` + `ClampToEdge` (Slice 5i)
//!
//! Glyph sample positions rarely land exactly on atlas texel centers —
//! subpixel positioning and non-integer pixel ratios are normal in a
//! DPR-aware renderer. `Linear` filtering across the four neighbouring
//! texels gives smooth edges; `Nearest` would alias visibly on a
//! fractional offset. Slice 5h's bind-group entry 2 is
//! `Sampler(Filtering)`, which *requires* `Linear` here in lockstep.
//!
//! Glyphs are packed contiguously in the atlas — adjacent glyphs share
//! atlas-texel boundaries. Any sample outside `[0, 1]` UV bleeds into a
//! neighbour and prints a sliver of the wrong glyph at the edge of a
//! quad. `ClampToEdge` is the only addressing mode that doesn't reach
//! into a packed neighbour: `Repeat` wraps to the opposite edge,
//! `MirrorRepeat` reflects, and clamp-to-border requires a border
//! color we don't want to supply for a packed atlas.

/// Build the [`wgpu::TextureDescriptor`] for the glyph atlas at a given
/// `(width, height)`. Pure function — no I/O, no GPU calls. Hands the
/// caller a descriptor ready for `device.create_texture(&desc)`.
///
/// See module docs for the WHY behind the `R8Unorm` +
/// `COPY_DST | TEXTURE_BINDING` + `mip=1 / sample=1 / D2` choices.
///
/// The label is a `'static` `Some("glyph_atlas")` so the return type can
/// be `TextureDescriptor<'static>` and the same builder serves both the
/// initial allocation and any future resize.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/glyph-atlas-texture-descriptor-slice-5e.md#interface
/// @issue #1754
pub fn glyph_atlas_texture_descriptor(width: u32, height: u32) -> wgpu::TextureDescriptor<'static> {
    wgpu::TextureDescriptor {
        label: Some("glyph_atlas"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::R8Unorm,
        usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    }
}

/// Build the [`wgpu::SamplerDescriptor`] used to sample the glyph
/// atlas. `Linear` mag/min filter + `ClampToEdge` addressing on every
/// axis. Pure function — no I/O, no GPU calls. See the module-level
/// docs for the WHY behind the `Linear` + `ClampToEdge` choices
/// (subpixel anti-alias + no cross-glyph bleed in a packed atlas).
///
/// `mipmap_filter`, LOD clamps, `anisotropy_clamp`, `compare`, and
/// `border_color` are pinned to the values consistent with a single-
/// mip 2D atlas: no mip filtering, LOD locked at 0, no anisotropy, no
/// depth-compare, no border. They are named explicitly so a future
/// wgpu rev that changes a `Default` doesn't silently shift our
/// behaviour.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/glyph-atlas-sampler-descriptor-slice-5i.md#interface
/// @issue #1758
pub fn glyph_atlas_sampler_descriptor() -> wgpu::SamplerDescriptor<'static> {
    wgpu::SamplerDescriptor {
        label: Some("glyph_atlas_sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        // The atlas is single-mip; nothing to filter across, so pick
        // the cheapest mode and pin LOD at 0.
        mipmap_filter: wgpu::FilterMode::Nearest,
        lod_min_clamp: 0.0,
        lod_max_clamp: 0.0,
        compare: None,
        anisotropy_clamp: 1,
        border_color: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_is_r8unorm() {
        // AC anchor: R8Unorm matches the 8-bit grayscale alpha from
        // Slice 5c's rasterizer. A refactor that switches to Rg8 / Rgba8
        // breaks this test before it breaks the upload.
        let desc = glyph_atlas_texture_descriptor(256, 256);
        assert_eq!(desc.format, wgpu::TextureFormat::R8Unorm);
    }

    #[test]
    fn usage_is_copy_dst_plus_texture_binding() {
        // AC anchor: COPY_DST for queue.write_texture (Slice 5f) +
        // TEXTURE_BINDING for the bind group (Slice 5h). No
        // RENDER_ATTACHMENT, no STORAGE_BINDING.
        let desc = glyph_atlas_texture_descriptor(256, 256);
        assert_eq!(
            desc.usage,
            wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        assert!(!desc.usage.contains(wgpu::TextureUsages::RENDER_ATTACHMENT));
        assert!(!desc.usage.contains(wgpu::TextureUsages::STORAGE_BINDING));
    }

    #[test]
    fn mip_and_sample_are_one() {
        let desc = glyph_atlas_texture_descriptor(256, 256);
        assert_eq!(desc.mip_level_count, 1);
        assert_eq!(desc.sample_count, 1);
    }

    #[test]
    fn dimension_is_d2() {
        let desc = glyph_atlas_texture_descriptor(256, 256);
        assert_eq!(desc.dimension, wgpu::TextureDimension::D2);
    }

    #[test]
    fn size_matches_input() {
        // Pin (width, height) → Extent3d mapping, and pin
        // depth_or_array_layers = 1 (the atlas is flat, not layered).
        let desc = glyph_atlas_texture_descriptor(512, 1024);
        assert_eq!(desc.size.width, 512);
        assert_eq!(desc.size.height, 1024);
        assert_eq!(desc.size.depth_or_array_layers, 1);
    }

    #[test]
    fn label_is_glyph_atlas() {
        // Label is `'static` per the descriptor lifetime; downstream
        // GPU debuggers (RenderDoc / Xcode capture) key off this name.
        let desc = glyph_atlas_texture_descriptor(256, 256);
        assert_eq!(desc.label, Some("glyph_atlas"));
    }

    #[test]
    fn view_formats_is_empty() {
        // No alternate view formats — the atlas is read as R8Unorm and
        // nothing else. Surfacing a non-empty list here would let a
        // future caller create a view with a mismatched format.
        let desc = glyph_atlas_texture_descriptor(256, 256);
        assert!(desc.view_formats.is_empty());
    }

    #[test]
    fn builder_is_pure_and_repeatable() {
        // Pin that calling the function twice with the same args yields
        // a structurally-equal descriptor (no hidden state, no global
        // counter). The label is `'static` so equality is meaningful.
        let a = glyph_atlas_texture_descriptor(64, 64);
        let b = glyph_atlas_texture_descriptor(64, 64);
        assert_eq!(a.label, b.label);
        assert_eq!(a.size, b.size);
        assert_eq!(a.mip_level_count, b.mip_level_count);
        assert_eq!(a.sample_count, b.sample_count);
        assert_eq!(a.dimension, b.dimension);
        assert_eq!(a.format, b.format);
        assert_eq!(a.usage, b.usage);
        assert_eq!(a.view_formats, b.view_formats);
    }

    // -------- Slice 5i (#1758): sampler descriptor tests --------

    #[test]
    fn sampler_label_is_glyph_atlas_sampler() {
        // AC anchor: label is `'static` so the return type can be
        // `SamplerDescriptor<'static>` and GPU debuggers key off a
        // stable name.
        let desc = glyph_atlas_sampler_descriptor();
        assert_eq!(desc.label, Some("glyph_atlas_sampler"));
    }

    #[test]
    fn sampler_filters_are_linear() {
        // AC anchor: mag_filter + min_filter = Linear.
        let desc = glyph_atlas_sampler_descriptor();
        assert_eq!(desc.mag_filter, wgpu::FilterMode::Linear);
        assert_eq!(desc.min_filter, wgpu::FilterMode::Linear);
    }

    #[test]
    fn sampler_address_modes_are_clamp_to_edge() {
        // AC anchor: address_mode_u/v/w = ClampToEdge — the only mode
        // that doesn't bleed into a packed neighbour glyph.
        let desc = glyph_atlas_sampler_descriptor();
        assert_eq!(desc.address_mode_u, wgpu::AddressMode::ClampToEdge);
        assert_eq!(desc.address_mode_v, wgpu::AddressMode::ClampToEdge);
        assert_eq!(desc.address_mode_w, wgpu::AddressMode::ClampToEdge);
    }

    #[test]
    fn sampler_mip_lod_pinned_for_single_mip() {
        // Pin the single-mip-aligned defaults: nothing to filter
        // across, LOD locked at 0, no anisotropy, no depth-compare.
        // A future wgpu rev that changes a `Default` will not silently
        // shift our behaviour because we name every field explicitly.
        let desc = glyph_atlas_sampler_descriptor();
        assert_eq!(desc.mipmap_filter, wgpu::FilterMode::Nearest);
        assert_eq!(desc.lod_min_clamp, 0.0);
        assert_eq!(desc.lod_max_clamp, 0.0);
        assert_eq!(desc.anisotropy_clamp, 1);
        assert!(desc.compare.is_none());
        assert!(desc.border_color.is_none());
    }

    #[test]
    fn sampler_builder_is_pure_and_repeatable() {
        // Same invariant as the texture descriptor: two calls produce
        // the same descriptor (no hidden state, no env-driven branch).
        let a = glyph_atlas_sampler_descriptor();
        let b = glyph_atlas_sampler_descriptor();
        assert_eq!(a.label, b.label);
        assert_eq!(a.mag_filter, b.mag_filter);
        assert_eq!(a.min_filter, b.min_filter);
        assert_eq!(a.address_mode_u, b.address_mode_u);
        assert_eq!(a.address_mode_v, b.address_mode_v);
        assert_eq!(a.address_mode_w, b.address_mode_w);
    }
}
