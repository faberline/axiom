//! Text pass — Slice 5g (#1756).
//!
//! WGSL source + per-instance struct for the text rendering pass.
//! Mirrors `cell_rect.wgsl`'s shape (viewport uniform on `@group(0)
//! @binding(0)`, 4-vertex bit-trick triangle strip, `#[repr(C)]`
//! per-instance struct with `Pod + Zeroable`) so the pipeline builder
//! and the render-pass orchestrator can treat text and cell-rect as
//! sibling pipelines that share the viewport bind group.
//!
//! ## Why these binding numbers
//!
//! `@group(0) @binding(0)` is the viewport uniform — byte-identical to
//! the one [`crate::cell_rect::CELL_RECT_WGSL`] uses (`size_px` +
//! `scroll_px`, 16 bytes total). Sharing the layout means the renderer
//! uploads the viewport once per frame and binds the same uniform
//! buffer to both pipelines.
//!
//! `@binding(1)` is the atlas (`texture_2d<f32>`) and `@binding(2)` is
//! the sampler. The shader pins these binding numbers so the upcoming
//! Slice 5h (bind-group layout) and Slice 5i (sampler descriptor) have
//! a fixed contract to author against. Putting them on the same group
//! as the viewport keeps the text pipeline at a single bind-group
//! descriptor — simpler than a two-group split that buys no perf at
//! this scale.
//!
//! ## Why `textureSample(...).r * color`, not premultiplied
//!
//! The atlas is `R8Unorm` — one channel, alpha. `textureSample` returns
//! a `vec4<f32>` with the sampled value in `.r` and 0 elsewhere, so the
//! shader reads `.r` explicitly. Multiplying by `color` straight (not
//! `color.rgb * color.a` premultiplied) matches the pipeline blend
//! state, which uses `BlendState::ALPHA_BLENDING` — the same
//! src.rgb * src.a + dst * (1 - src.a) convention `cell_rect` already
//! relies on. A future slice can introduce premultiplied output if
//! profiling demands.
//!
//! ## Why normalized UVs in the per-instance struct
//!
//! `textureSample` takes normalized UVs natively. If we passed pixel
//! coordinates, every fragment would re-divide by the atlas dimensions
//! to convert — that's per-fragment overhead the atlas allocator can
//! eliminate by dividing once per glyph at upload time. Storing
//! `uv_min` + `uv_max` (instead of `uv_origin` + `uv_size`) saves the
//! per-fragment `min + corner * size` and matches the screen-space
//! `pos_px + corner * size_px` shape directly.
//!
//! ## Why no per-vertex buffer
//!
//! `vs_main` expands `@builtin(vertex_index)` 0..4 into the four
//! triangle-strip corners via the bit trick
//! `(vid & 1, (vid >> 1) & 1)` — produces `(0,0)(1,0)(0,1)(1,1)` in
//! exactly that order. No vertex buffer means the pipeline takes one
//! instance buffer and nothing else; the render-pass orchestrator can
//! flip between cell-rect and text pipelines without re-binding a
//! vertex buffer.

use bytemuck::{Pod, Zeroable};

/// WGSL source for the text pass (entry points `vs_main` / `fs_main`).
///
/// See module docs for the WHY behind the binding-number contract, the
/// straight-RGBA multiplication, the normalized-UV per-instance layout,
/// and the no-per-vertex-buffer bit-trick.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/text-pass-wgsl-slice-5g.md#interface
/// @issue #1756
pub const TEXT_PASS_WGSL: &str = r#"
struct Viewport {
    size_px:   vec2<f32>,
    scroll_px: vec2<f32>,
};

@group(0) @binding(0) var<uniform> viewport: Viewport;
@group(0) @binding(1) var atlas: texture_2d<f32>;
@group(0) @binding(2) var samp:  sampler;

struct GlyphInstance {
    @location(0) pos_px:  vec2<f32>,
    @location(1) size_px: vec2<f32>,
    @location(2) uv_min:  vec2<f32>,
    @location(3) uv_max:  vec2<f32>,
    @location(4) color:   vec4<f32>,
};

struct VsOut {
    @builtin(position) clip:  vec4<f32>,
    @location(0)       uv:    vec2<f32>,
    @location(1)       color: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vid: u32, inst: GlyphInstance) -> VsOut {
    // 0..3 -> (0,0)(1,0)(0,1)(1,1) -- triangle strip, same bit trick
    // as cell_rect.wgsl.
    let corner = vec2<f32>(f32(vid & 1u), f32((vid >> 1u) & 1u));
    // Screen-space pixel position, then subtract scroll (virtual-sheet
    // coords -> visible-window coords) and convert to NDC.
    let px = inst.pos_px + corner * inst.size_px - viewport.scroll_px;
    let ndc = vec2<f32>(
        px.x / viewport.size_px.x * 2.0 - 1.0,
        1.0 - px.y / viewport.size_px.y * 2.0,
    );
    // Atlas-space UV: lerp between uv_min and uv_max by the same
    // corner. Pre-divided by atlas dims at allocator time so the
    // shader does no per-fragment division.
    let uv = mix(inst.uv_min, inst.uv_max, corner);

    var out: VsOut;
    out.clip  = vec4<f32>(ndc, 0.0, 1.0);
    out.uv    = uv;
    out.color = inst.color;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // R8Unorm alpha lands in .r; multiply by the per-glyph straight
    // RGBA so the pipeline's ALPHA_BLENDING produces correct over-blend
    // against the existing framebuffer.
    let alpha = textureSample(atlas, samp, in.uv).r;
    return in.color * alpha;
}
"#;

/// Per-instance attributes for one glyph quad.
///
/// `pos_px` is the top-left corner in physical pixels (Y down),
/// `size_px` is width/height, `uv_min` / `uv_max` are the atlas-space
/// UV rect in normalized coordinates (0..=1), and `color` is straight
/// (un-premultiplied) RGBA in 0..=1.
///
/// Field order matches the WGSL `GlyphInstance` struct in
/// [`TEXT_PASS_WGSL`]; the GPU reads the bytes in declaration order via
/// the vertex layout, not by name. Changing the order requires
/// updating both sides in lockstep.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/text-pass-wgsl-slice-5g.md#interface
/// @issue #1756
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct GlyphInstance {
    pub pos_px: [f32; 2],
    pub size_px: [f32; 2],
    pub uv_min: [f32; 2],
    pub uv_max: [f32; 2],
    pub color: [f32; 4],
}

// -------- Slice 5h (#1757): bind-group layout entries --------
//
// Why a `pub const` array (not a function): matches the cell-rect
// pass's `VIEWPORT_BGL_ENTRIES` precedent and keeps the entries
// `'static`-borrowable so the composing descriptor below can hand
// `&TEXT_PASS_BIND_GROUP_ENTRIES` to wgpu directly.
//
// Why VERTEX-only on the viewport and FRAGMENT-only on the
// texture/sampler: wgpu validates that a binding's `visibility` mask
// covers exactly the stages that actually read the binding. Naming
// `VERTEX | FRAGMENT` on the texture entry would silently pass today
// but trip validation as soon as a shader edit removes the read.
// Minimum-surface visibility surfaces shader/layout drift loud.
//
// Why `TextureSampleType::Float { filterable: true }`: the atlas is
// `R8Unorm` (8-bit unsigned normalized → `f32` on shader read) AND
// the sampler is `Filtering` (entry 2). `Filtering` samplers require
// the texture binding to be `filterable: true`; otherwise wgpu
// rejects the bind-group at creation time. Slice 5e (#1754) already
// commits us to `R8Unorm` + `sample_count = 1`, both of which map to
// `Float { filterable: true }` + `multisampled: false`.

/// Bind-group layout entries for the text pass — viewport (vertex
/// uniform), atlas (fragment texture), sampler (fragment filtering).
///
/// Pinned in binding-number order so a future composer can index by
/// position. See the module-level docs and the surrounding internal
/// comments for the WHY behind the visibility split and the
/// `Float { filterable: true }` + `Filtering` pairing.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/text-pass-bind-group-layout-slice-5h.md#interface
/// @issue #1757
pub const TEXT_PASS_BIND_GROUP_ENTRIES: [wgpu::BindGroupLayoutEntry; 3] = [
    // @binding(0) — viewport uniform (size_px + scroll_px), read by
    // vs_main only.
    wgpu::BindGroupLayoutEntry {
        binding: 0,
        visibility: wgpu::ShaderStages::VERTEX,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    },
    // @binding(1) — glyph atlas (R8Unorm, D2), sampled by fs_main.
    wgpu::BindGroupLayoutEntry {
        binding: 1,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    },
    // @binding(2) — linear sampler, bound by fs_main's
    // `textureSample(atlas, samp, uv)` call.
    wgpu::BindGroupLayoutEntry {
        binding: 2,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        count: None,
    },
];

/// `BindGroupLayoutDescriptor` composing [`TEXT_PASS_BIND_GROUP_ENTRIES`]
/// with the `'static` label `"text_pass_bgl"`, ready for
/// `device.create_bind_group_layout(&desc)`.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/text-pass-bind-group-layout-slice-5h.md#interface
/// @issue #1757
pub fn text_pass_bind_group_layout_descriptor() -> wgpu::BindGroupLayoutDescriptor<'static> {
    wgpu::BindGroupLayoutDescriptor {
        label: Some("text_pass_bgl"),
        entries: &TEXT_PASS_BIND_GROUP_ENTRIES,
    }
}

/// `VertexBufferLayout` describing one instance buffer holding
/// `GlyphInstance` records (step mode = `Instance`). Mirrors the
/// shape of `cell_rect::cell_rect_vertex_layout` so the text pass and
/// cell pass share a no-per-vertex-buffer convention.
///
/// @spec .aw/tech-design/projects/jet/specs/webgpu-text-pass-encode.md#changes
/// @issue #2191
pub fn text_pass_vertex_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<GlyphInstance>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &GLYPH_INSTANCE_ATTRIBUTES,
    }
}

const GLYPH_INSTANCE_ATTRIBUTES: [wgpu::VertexAttribute; 5] = [
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x2,
        offset: 0,
        shader_location: 0,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x2,
        offset: 8,
        shader_location: 1,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x2,
        offset: 16,
        shader_location: 2,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x2,
        offset: 24,
        shader_location: 3,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x4,
        offset: 32,
        shader_location: 4,
    },
];

/// Build a text-pass `wgpu::RenderPipeline` targeting `format` with
/// `msaa_count` sample count, using `pipeline_layout` (which must
/// reference a BGL built from [`text_pass_bind_group_layout_descriptor`])
/// and `shader` (built from [`TEXT_PASS_WGSL`]).
///
/// Shape matches `WebGpuRenderer::create_cell_pipeline` — same
/// triangle-strip + `ALPHA_BLENDING` + MSAA-aware multisample state —
/// so the renderer's text-pass cache is keyed the same way and a
/// drop-in pair with the cell pipeline.
///
/// @spec .aw/tech-design/projects/jet/specs/webgpu-text-pass-encode.md#changes
/// @issue #2191
pub fn create_text_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    msaa_count: u32,
    pipeline_layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("text_pass_pipeline"),
        layout: Some(pipeline_layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            buffers: &[text_pass_vertex_layout()],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleStrip,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState {
            count: msaa_count,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview: None,
        cache: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wgsl_parses_via_naga() {
        // AC anchor: shader source must be valid WGSL — fail fast
        // before we ever touch a GPU. naga's parse is the same path
        // wgpu uses internally.
        let module = naga::front::wgsl::parse_str(TEXT_PASS_WGSL)
            .expect("TEXT_PASS_WGSL must parse cleanly through naga");
        let names: Vec<_> = module
            .entry_points
            .iter()
            .map(|e| e.name.as_str())
            .collect();
        assert!(
            names.contains(&"vs_main"),
            "missing vs_main entry point: {names:?}"
        );
        assert!(
            names.contains(&"fs_main"),
            "missing fs_main entry point: {names:?}"
        );
    }

    #[test]
    fn bindings_match_ac() {
        // AC anchor: @group(0) @binding(0) viewport uniform,
        // @binding(1) atlas texture, @binding(2) sampler. Parse the
        // module and assert the global vars expose those exact
        // (group, binding) pairs.
        let module = naga::front::wgsl::parse_str(TEXT_PASS_WGSL)
            .expect("TEXT_PASS_WGSL must parse cleanly");
        let mut bindings: Vec<(u32, u32)> = module
            .global_variables
            .iter()
            .filter_map(|(_, gv)| gv.binding.as_ref().map(|b| (b.group, b.binding)))
            .collect();
        bindings.sort();
        assert_eq!(
            bindings,
            vec![(0, 0), (0, 1), (0, 2)],
            "bindings must be (group=0, binding=0/1/2)"
        );
    }

    #[test]
    fn glyph_instance_layout() {
        // AC invariant: WGSL GlyphInstance field order must match the
        // Rust struct's #[repr(C)] order. Sizes are 2+2+2+2+4 f32s =
        // 12 floats = 48 bytes.
        assert_eq!(size_of::<GlyphInstance>(), 12 * size_of::<f32>());
        // Spot-check the byte offsets via std::mem::offset_of! to pin
        // the declaration order — a refactor that reorders fields
        // breaks this test before it breaks the GPU.
        assert_eq!(std::mem::offset_of!(GlyphInstance, pos_px), 0);
        assert_eq!(std::mem::offset_of!(GlyphInstance, size_px), 8);
        assert_eq!(std::mem::offset_of!(GlyphInstance, uv_min), 16);
        assert_eq!(std::mem::offset_of!(GlyphInstance, uv_max), 24);
        assert_eq!(std::mem::offset_of!(GlyphInstance, color), 32);
    }

    #[test]
    fn fs_main_multiplies_alpha_by_color() {
        // Soft anchor for the fs_main rule. Naga's IR is too low-level
        // to introspect the expression tree cheaply, so we do a string
        // grep on the WGSL source for the contract phrase. Any future
        // edit that drops the alpha multiplication breaks this.
        assert!(
            TEXT_PASS_WGSL.contains("textureSample(atlas, samp, in.uv).r"),
            "fs_main must sample the atlas .r channel"
        );
        assert!(
            TEXT_PASS_WGSL.contains("in.color * alpha"),
            "fs_main must multiply alpha by the per-glyph color"
        );
    }

    #[test]
    fn vs_main_expands_triangle_strip_via_bit_trick() {
        // Pins the 4-vertex triangle strip contract — vs_main must use
        // the same (vid & 1, (vid >> 1) & 1) corner expansion as
        // cell_rect. The render-pass orchestrator switches between the
        // two pipelines without re-binding a vertex buffer; both must
        // therefore agree on the no-per-vertex-buffer convention.
        assert!(
            TEXT_PASS_WGSL.contains("vid & 1u"),
            "vs_main must use the bit-trick corner expansion"
        );
        assert!(
            TEXT_PASS_WGSL.contains("(vid >> 1u) & 1u"),
            "vs_main must use the bit-trick corner expansion"
        );
    }

    #[test]
    fn glyph_instance_is_pod_zeroable() {
        // Pin the bytemuck contract — Pod + Zeroable + Copy is what the
        // instance buffer upload relies on. A refactor that adds a
        // non-Pod field (e.g. a String) trips this test at compile
        // time, not at the buffer upload.
        fn assert_pod<T: bytemuck::Pod + bytemuck::Zeroable + Copy>() {}
        assert_pod::<GlyphInstance>();
        // And the default value is all-zeros.
        let z = GlyphInstance::default();
        assert_eq!(z.pos_px, [0.0, 0.0]);
        assert_eq!(z.color, [0.0, 0.0, 0.0, 0.0]);
    }

    // -------- Slice 5h (#1757): bind-group layout tests --------

    #[test]
    fn bind_group_entries_count_is_three() {
        // AC anchor: TEXT_PASS_BIND_GROUP_ENTRIES has exactly three
        // entries (viewport, atlas, sampler) in binding-number order.
        assert_eq!(TEXT_PASS_BIND_GROUP_ENTRIES.len(), 3);
        assert_eq!(TEXT_PASS_BIND_GROUP_ENTRIES[0].binding, 0);
        assert_eq!(TEXT_PASS_BIND_GROUP_ENTRIES[1].binding, 1);
        assert_eq!(TEXT_PASS_BIND_GROUP_ENTRIES[2].binding, 2);
    }

    #[test]
    fn entry_zero_is_viewport_uniform_vertex() {
        // AC anchor: entry 0 — Uniform buffer, VERTEX-only.
        let entry = TEXT_PASS_BIND_GROUP_ENTRIES[0];
        assert_eq!(entry.visibility, wgpu::ShaderStages::VERTEX);
        match entry.ty {
            wgpu::BindingType::Buffer {
                ty,
                has_dynamic_offset,
                ..
            } => {
                assert_eq!(ty, wgpu::BufferBindingType::Uniform);
                assert!(!has_dynamic_offset);
            }
            other => panic!("entry 0 must be a Uniform buffer, got {other:?}"),
        }
        assert!(entry.count.is_none());
    }

    #[test]
    fn entry_one_is_atlas_texture_fragment_d2() {
        // AC anchor: entry 1 — Texture, FRAGMENT-only, view_dimension
        // D2, filterable Float sample type (matches Slice 5e's R8Unorm
        // + sample_count = 1).
        let entry = TEXT_PASS_BIND_GROUP_ENTRIES[1];
        assert_eq!(entry.visibility, wgpu::ShaderStages::FRAGMENT);
        match entry.ty {
            wgpu::BindingType::Texture {
                sample_type,
                view_dimension,
                multisampled,
            } => {
                assert_eq!(
                    sample_type,
                    wgpu::TextureSampleType::Float { filterable: true },
                );
                assert_eq!(view_dimension, wgpu::TextureViewDimension::D2);
                assert!(!multisampled);
            }
            other => panic!("entry 1 must be a Texture binding, got {other:?}"),
        }
    }

    #[test]
    fn entry_two_is_sampler_fragment_filtering() {
        // AC anchor: entry 2 — Sampler, FRAGMENT-only, Filtering
        // category. Pairs with entry 1's `filterable: true`.
        let entry = TEXT_PASS_BIND_GROUP_ENTRIES[2];
        assert_eq!(entry.visibility, wgpu::ShaderStages::FRAGMENT);
        assert_eq!(
            entry.ty,
            wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        );
    }

    #[test]
    fn descriptor_carries_static_label() {
        // Pin the descriptor's label so GPU debuggers (RenderDoc /
        // Xcode capture) key off a stable name across the renderer's
        // lifetime.
        let desc = text_pass_bind_group_layout_descriptor();
        assert_eq!(desc.label, Some("text_pass_bgl"));
        // The entries slice points at our const — same length, same
        // binding numbers.
        assert_eq!(desc.entries.len(), 3);
        assert_eq!(desc.entries[0].binding, 0);
        assert_eq!(desc.entries[1].binding, 1);
        assert_eq!(desc.entries[2].binding, 2);
    }
}
