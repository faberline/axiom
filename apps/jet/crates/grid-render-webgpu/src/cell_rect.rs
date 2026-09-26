//! Cell-rect rendering primitives — WGSL source, instance layout, bind-group
//! and pipeline-config descriptors.
//!
//! Why this module exists: every later slice of the WebGPU runtime (frame
//! loop, render-pass orchestration, instance-buffer pool) needs to agree on
//! exactly one cell-rect render contract — the shader entry points, the
//! per-instance attribute layout, and the bind-group shape. Centralising
//! those primitives here keeps the contract single-sourced; the pipeline
//! builder in [`crate::lib`] just composes them.
//!
//! Invariant: the WGSL `CellInstance` struct order MUST match
//! [`CellInstance`]'s `#[repr(C)]` field order — the GPU reads the bytes
//! in declaration order via the vertex layout, not by name.

use std::mem::size_of;

use bytemuck::{Pod, Zeroable};

/// WGSL source for the cell-rect shader (entry points `vs_main`/`fs_main`).
///
/// Generates the quad corners from `@builtin(vertex_index)` 0..3 via bit
/// tricks so no per-vertex vertex buffer is needed — only the per-instance
/// buffer described by [`cell_rect_vertex_layout`].
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/cell-rect-pipeline-slice-4b.md#interface
/// @issue #1720
pub const CELL_RECT_WGSL: &str = r#"
struct Viewport {
    size_px:   vec2<f32>,
    scroll_px: vec2<f32>,
};

@group(0) @binding(0) var<uniform> viewport: Viewport;

struct CellInstance {
    @location(0) pos_px:  vec2<f32>,
    @location(1) size_px: vec2<f32>,
    @location(2) color:   vec4<f32>,
};

struct VsOut {
    @builtin(position) clip:  vec4<f32>,
    @location(0)       color: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vid: u32, inst: CellInstance) -> VsOut {
    // 0..3 -> (0,0)(1,0)(0,1)(1,1) -- triangle strip
    let corner = vec2<f32>(f32(vid & 1u), f32((vid >> 1u) & 1u));
    // Translate from virtual-sheet coords into the visible window by
    // subtracting the scroll offset. Slice 4s (#1737) — scroll is a
    // uniform-only update; instance positions stay in virtual coords.
    let px = inst.pos_px + corner * inst.size_px - viewport.scroll_px;
    let ndc = vec2<f32>(
        px.x / viewport.size_px.x * 2.0 - 1.0,
        1.0 - px.y / viewport.size_px.y * 2.0,
    );
    var out: VsOut;
    out.clip  = vec4<f32>(ndc, 0.0, 1.0);
    out.color = inst.color;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return in.color;
}
"#;

/// Per-instance attributes for one cell rectangle.
///
/// `pos_px` is the top-left corner in physical pixels (Y down), `size_px`
/// is width/height, `color` is straight (un-premultiplied) RGBA in 0..=1.
/// Field order matches the WGSL `CellInstance` struct in [`CELL_RECT_WGSL`];
/// changing it requires updating both sides in lockstep.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/cell-rect-pipeline-slice-4b.md#interface
/// @issue #1720
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct CellInstance {
    pub pos_px: [f32; 2],
    pub size_px: [f32; 2],
    pub color: [f32; 4],
}

/// Shared pipeline configuration (primitive topology, blend mode, vertex
/// count per instance). Separated from the rest so later runtime slices
/// — for example a render-pass orchestrator — can query the topology
/// without rebuilding the pipeline.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/cell-rect-pipeline-slice-4b.md#interface
/// @issue #1720
#[derive(Clone, Debug)]
pub struct PipelineConfig {
    pub primitive: wgpu::PrimitiveState,
    pub blend: wgpu::BlendState,
    pub vertices_per_instance: u32,
}

const VIEWPORT_BGL_ENTRIES: [wgpu::BindGroupLayoutEntry; 1] = [wgpu::BindGroupLayoutEntry {
    binding: 0,
    visibility: wgpu::ShaderStages::VERTEX,
    ty: wgpu::BindingType::Buffer {
        ty: wgpu::BufferBindingType::Uniform,
        has_dynamic_offset: false,
        min_binding_size: None,
    },
    count: None,
}];

const CELL_INSTANCE_ATTRIBUTES: [wgpu::VertexAttribute; 3] = wgpu::vertex_attr_array![
    0 => Float32x2, // pos_px
    1 => Float32x2, // size_px
    2 => Float32x4, // color
];

/// `BindGroupLayoutDescriptor` for the single viewport uniform binding
/// the vertex shader reads.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/cell-rect-pipeline-slice-4b.md#interface
/// @issue #1720
pub fn cell_rect_bind_group_layout_descriptor() -> wgpu::BindGroupLayoutDescriptor<'static> {
    wgpu::BindGroupLayoutDescriptor {
        label: Some("cell_rect_viewport_bgl"),
        entries: &VIEWPORT_BGL_ENTRIES,
    }
}

/// `VertexBufferLayout` describing one instance buffer holding
/// `CellInstance` records (step mode = `Instance`).
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/cell-rect-pipeline-slice-4b.md#interface
/// @issue #1720
pub fn cell_rect_vertex_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: size_of::<CellInstance>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &CELL_INSTANCE_ATTRIBUTES,
    }
}

/// Pipeline shared config: TriangleStrip with 4 vertices/instance, standard
/// alpha-blend over, no depth/stencil.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/cell-rect-pipeline-slice-4b.md#interface
/// @issue #1720
pub fn cell_rect_pipeline_config() -> PipelineConfig {
    PipelineConfig {
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleStrip,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        },
        blend: wgpu::BlendState::ALPHA_BLENDING,
        vertices_per_instance: 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wgsl_parses_via_naga() {
        // AC: shader source must be valid WGSL — fail fast before we ever
        // touch a GPU. naga's parse is the same path wgpu uses internally.
        let module = naga::front::wgsl::parse_str(CELL_RECT_WGSL)
            .expect("CELL_RECT_WGSL must parse cleanly");
        // vs_main + fs_main are the contracted entry points.
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
    fn cell_instance_layout_matches_attributes() {
        // AC invariant: WGSL CellInstance field order must match the Rust
        // struct's #[repr(C)] order. If a future edit reorders fields on
        // one side, the offsets here drift and this test trips.
        let stride = size_of::<CellInstance>() as wgpu::BufferAddress;
        let layout = cell_rect_vertex_layout();
        assert_eq!(layout.array_stride, stride);
        assert_eq!(layout.step_mode, wgpu::VertexStepMode::Instance);
        assert_eq!(layout.attributes.len(), 3);
        // pos_px @ offset 0, size_px @ offset 8, color @ offset 16
        assert_eq!(layout.attributes[0].shader_location, 0);
        assert_eq!(layout.attributes[0].offset, 0);
        assert_eq!(layout.attributes[1].shader_location, 1);
        assert_eq!(layout.attributes[1].offset, 8);
        assert_eq!(layout.attributes[2].shader_location, 2);
        assert_eq!(layout.attributes[2].offset, 16);
        assert_eq!(stride, 32);
    }

    #[test]
    fn bind_group_layout_has_single_viewport_uniform() {
        let d = cell_rect_bind_group_layout_descriptor();
        assert_eq!(d.entries.len(), 1);
        let e = &d.entries[0];
        assert_eq!(e.binding, 0);
        assert!(e.visibility.contains(wgpu::ShaderStages::VERTEX));
        match e.ty {
            wgpu::BindingType::Buffer {
                ty,
                has_dynamic_offset,
                ..
            } => {
                assert_eq!(ty, wgpu::BufferBindingType::Uniform);
                assert!(!has_dynamic_offset);
            }
            other => panic!("expected uniform buffer binding, got {other:?}"),
        }
    }

    #[test]
    fn pipeline_config_is_triangle_strip_4_verts() {
        let cfg = cell_rect_pipeline_config();
        assert_eq!(
            cfg.primitive.topology,
            wgpu::PrimitiveTopology::TriangleStrip
        );
        assert_eq!(cfg.vertices_per_instance, 4);
        // Standard alpha-over blending.
        assert_eq!(cfg.blend, wgpu::BlendState::ALPHA_BLENDING);
    }
}
