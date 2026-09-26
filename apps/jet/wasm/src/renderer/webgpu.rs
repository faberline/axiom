// SPEC-MANAGED: .aw/tech-design/projects/jet/semantic/jet-wasm-src-renderer.md#schema
// CODEGEN-BEGIN
//! WebGPU paint-planning backend.
//!
//! This module keeps Jet's renderer pipeline unchanged: layout emits a
//! `LayoutTree`, paint emits `PaintOp`s, and backends consume that op stream.
//! The WebGPU backend translates the subset that already matches the shared
//! grid renderer's cell and text pipelines into `CellInstance`s and structured
//! text runs. The wasm bridge shapes those text runs into real glyph atlas
//! instances before calling `jet_grid_render_webgpu::WebGpuRenderer`.

use jet_grid_render_webgpu::cell_rect::CellInstance;

use super::{Color, PaintBackend, PaintOp, Rect};

/// Packed JS wire stride used by `jet-grid-wasm::RendererHandle`.
///
/// Order: `pos_px.xy`, `size_px.xy`, `color.rgba`.
pub const CELL_F32_STRIDE: usize = 8;

/// One frame of WebGPU-ready primitive data.
///
/// `cells` maps directly to the shared `jet-grid-render-webgpu`
/// cell-rect instance buffer. `unsupported` records paint ops that need
/// later WebGPU passes before the backend can claim canvas parity.
/// @spec .aw/tech-design/projects/jet/semantic/jet-wasm-src-renderer.md#schema
#[derive(Debug, Clone, PartialEq)]
pub struct WebGpuFramePlan {
    pub cells: Vec<CellInstance>,
    pub text_runs: Vec<WebGpuTextRun>,
    pub unsupported: Vec<WebGpuUnsupportedOp>,
}

/// @spec .aw/tech-design/projects/jet/semantic/jet-wasm-src-renderer.md#schema
impl WebGpuFramePlan {
    pub fn is_complete(&self) -> bool {
        self.unsupported.is_empty()
    }

    pub fn packed_f32_len(&self) -> usize {
        self.cells.len() * CELL_F32_STRIDE
    }

    pub fn write_packed_f32(&self, out: &mut Vec<f32>) {
        out.reserve(self.packed_f32_len());
        for cell in &self.cells {
            out.extend_from_slice(&[
                cell.pos_px[0],
                cell.pos_px[1],
                cell.size_px[0],
                cell.size_px[1],
                cell.color[0],
                cell.color[1],
                cell.color[2],
                cell.color[3],
            ]);
        }
    }

    pub fn to_packed_f32(&self) -> Vec<f32> {
        let mut out = Vec::with_capacity(self.packed_f32_len());
        self.write_packed_f32(&mut out);
        out
    }

    #[cfg(target_arch = "wasm32")]
    pub fn to_float32_array(&self) -> js_sys::Float32Array {
        js_sys::Float32Array::from(self.to_packed_f32().as_slice())
    }

    #[cfg(target_arch = "wasm32")]
    pub fn to_js_text_runs(&self) -> js_sys::Array {
        let out = js_sys::Array::new();
        for run in &self.text_runs {
            out.push(&run.to_js_object());
        }
        out
    }
}

/// Text paint data preserved by the WebGPU planner.
///
/// The wasm bridge submits these runs through the WebGPU text pass after
/// shaping/rasterizing them into the shared glyph atlas. Text is therefore
/// part of the supported WebGPU plan; unsupported only tracks operations that
/// still have no WebGPU lowering.
/// @spec .aw/tech-design/projects/jet/semantic/jet-wasm-src-renderer.md#schema
#[derive(Debug, Clone, PartialEq)]
pub struct WebGpuTextRun {
    pub origin_px: [f32; 2],
    pub content: String,
    pub font_family: String,
    pub font_size_px: f32,
    pub font_weight: u16,
    pub color: [f32; 4],
}

/// @spec .aw/tech-design/projects/jet/semantic/jet-wasm-src-renderer.md#schema
#[cfg(target_arch = "wasm32")]
impl WebGpuTextRun {
    pub fn to_js_object(&self) -> wasm_bindgen::JsValue {
        use wasm_bindgen::JsValue;

        let obj = js_sys::Object::new();
        let origin = js_sys::Array::new();
        origin.push(&JsValue::from_f64(self.origin_px[0] as f64));
        origin.push(&JsValue::from_f64(self.origin_px[1] as f64));
        let color = js_sys::Array::new();
        for channel in self.color {
            color.push(&JsValue::from_f64(channel as f64));
        }

        let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("originPx"), origin.as_ref());
        let _ = js_sys::Reflect::set(
            &obj,
            &JsValue::from_str("content"),
            &JsValue::from_str(&self.content),
        );
        let _ = js_sys::Reflect::set(
            &obj,
            &JsValue::from_str("fontFamily"),
            &JsValue::from_str(&self.font_family),
        );
        let _ = js_sys::Reflect::set(
            &obj,
            &JsValue::from_str("fontSizePx"),
            &JsValue::from_f64(self.font_size_px as f64),
        );
        let _ = js_sys::Reflect::set(
            &obj,
            &JsValue::from_str("fontWeight"),
            &JsValue::from_f64(self.font_weight as f64),
        );
        let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("color"), color.as_ref());
        obj.into()
    }
}

/// Paint operations that the current WebGPU planner intentionally does not
/// lower yet.
///
/// `StrokeRect` was removed in #2117 — strokes now lower to up to four
/// `CellInstance` edge strips. Text lowers to structured text runs consumed by
/// the glyph atlas bridge. Clip still needs scissor/clip-stack work before it
/// can be drawn.
/// @spec .aw/tech-design/projects/jet/semantic/jet-wasm-src-renderer.md#schema
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebGpuUnsupportedOp {
    Clip,
}

/// `PaintBackend` implementation that records the latest WebGPU frame plan.
///
/// The backend is host-testable and does not acquire a GPU adapter itself.
/// Browser/native adapter selection is owned by
/// `jet_grid_render_webgpu::backend`; this module exposes its description
/// so the Jet layer reports the same backend contract as the lower renderer.
/// @spec .aw/tech-design/projects/jet/semantic/jet-wasm-src-renderer.md#schema
#[derive(Debug, Clone)]
pub struct WebGpuBackend {
    last_frame: WebGpuFramePlan,
    dpr: f32,
}

/// @spec .aw/tech-design/projects/jet/semantic/jet-wasm-src-renderer.md#schema
impl WebGpuBackend {
    pub fn new() -> Self {
        Self {
            last_frame: WebGpuFramePlan {
                cells: Vec::new(),
                text_runs: Vec::new(),
                unsupported: Vec::new(),
            },
            dpr: 1.0,
        }
    }

    pub fn plan(ops: &[PaintOp]) -> WebGpuFramePlan {
        Self::plan_with_dpr(ops, 1.0)
    }

    pub fn plan_with_dpr(ops: &[PaintOp], dpr: f32) -> WebGpuFramePlan {
        let dpr = if dpr.is_finite() && dpr > 0.0 {
            dpr
        } else {
            1.0
        };
        let mut cells = Vec::new();
        let mut text_runs = Vec::new();
        let mut unsupported = Vec::new();

        for op in ops {
            match op {
                PaintOp::FillRect { rect, color } => {
                    cells.push(fill_rect_to_cell(scale_rect(*rect, dpr), *color));
                }
                PaintOp::StrokeRect { rect, color, width } => {
                    cells.extend(lower_stroke_rect(
                        scale_rect(*rect, dpr),
                        *color,
                        *width * dpr,
                    ));
                }
                PaintOp::Text {
                    origin,
                    content,
                    font,
                    color,
                } => {
                    text_runs.push(WebGpuTextRun {
                        origin_px: [origin.x * dpr, origin.y * dpr],
                        content: content.clone(),
                        font_family: font.family.clone(),
                        font_size_px: font.size_px * dpr,
                        font_weight: font.weight,
                        color: color_to_f32(*color),
                    });
                }
                PaintOp::PushClip { .. } | PaintOp::PopClip => {
                    unsupported.push(WebGpuUnsupportedOp::Clip);
                }
            }
        }

        WebGpuFramePlan {
            cells,
            text_runs,
            unsupported,
        }
    }

    pub fn set_dpr(&mut self, dpr: f32) {
        self.dpr = if dpr.is_finite() && dpr > 0.0 {
            dpr
        } else {
            1.0
        };
    }

    pub fn last_frame(&self) -> &WebGpuFramePlan {
        &self.last_frame
    }

    pub fn backend_description(&self) -> &'static str {
        jet_grid_render_webgpu::backend::backend_description()
    }
}

/// @spec .aw/tech-design/projects/jet/semantic/jet-wasm-src-renderer.md#schema
impl Default for WebGpuBackend {
    fn default() -> Self {
        Self::new()
    }
}

/// @spec .aw/tech-design/projects/jet/semantic/jet-wasm-src-renderer.md#schema
impl PaintBackend for WebGpuBackend {
    fn execute(&mut self, ops: &[PaintOp]) {
        self.last_frame = Self::plan_with_dpr(ops, self.dpr);
    }
}

fn scale_rect(rect: Rect, scale: f32) -> Rect {
    Rect {
        x: rect.x * scale,
        y: rect.y * scale,
        w: rect.w * scale,
        h: rect.h * scale,
    }
}

fn fill_rect_to_cell(rect: Rect, color: Color) -> CellInstance {
    CellInstance {
        pos_px: [rect.x, rect.y],
        size_px: [rect.w, rect.h],
        color: color_to_f32(color),
    }
}

/// Lower one `PaintOp::StrokeRect` into up to four `CellInstance`s
/// forming a hollow rectangular outline.
///
/// Stroke is center-aligned on the rect path (canvas `strokeRect`
/// convention): the `width`-thick stroke straddles the path by `width/2`
/// on each side. Top + Bottom strips span the full horizontal extent
/// (including corner areas); Left + Right strips span only the middle
/// so the four strips tile without overlap — which keeps translucent
/// strokes from double-blending at corners.
///
/// Degenerate inputs emit zero cells:
/// - `width <= 0.0`, `rect.w <= 0.0`, or `rect.h <= 0.0` → empty.
/// - `width >= rect.h` → left/right collapse, only top + bottom remain.
/// - `width >= rect.w` → top/bottom collapse, only left + right remain.
///
/// @spec .aw/tech-design/projects/jet/logic/wasm-renderer-webgpu-strokerect.md#interfaces
/// @issue #2117
fn lower_stroke_rect(rect: Rect, color: Color, width: f32) -> Vec<CellInstance> {
    if width <= 0.0 || rect.w <= 0.0 || rect.h <= 0.0 {
        return Vec::new();
    }

    let half = width / 2.0;
    let rgba = color_to_f32(color);
    let mut out = Vec::with_capacity(4);

    let top_w = rect.w + width;
    let middle_h = rect.h - width;

    if top_w > 0.0 {
        out.push(CellInstance {
            pos_px: [rect.x - half, rect.y - half],
            size_px: [top_w, width],
            color: rgba,
        });
        out.push(CellInstance {
            pos_px: [rect.x - half, rect.y + rect.h - half],
            size_px: [top_w, width],
            color: rgba,
        });
    }
    if middle_h > 0.0 {
        out.push(CellInstance {
            pos_px: [rect.x - half, rect.y + half],
            size_px: [width, middle_h],
            color: rgba,
        });
        out.push(CellInstance {
            pos_px: [rect.x + rect.w - half, rect.y + half],
            size_px: [width, middle_h],
            color: rgba,
        });
    }

    out
}

fn color_to_f32(color: Color) -> [f32; 4] {
    [
        color.r as f32 / 255.0,
        color.g as f32 / 255.0,
        color.b as f32 / 255.0,
        color.a as f32 / 255.0,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer::{FontSpec, Point};

    #[test]
    fn fill_rect_lowers_to_cell_instance() {
        let plan = WebGpuBackend::plan(&[PaintOp::FillRect {
            rect: Rect {
                x: 10.0,
                y: 20.0,
                w: 30.0,
                h: 40.0,
            },
            color: Color {
                r: 128,
                g: 64,
                b: 255,
                a: 127,
            },
        }]);

        assert!(plan.is_complete());
        assert_eq!(plan.cells.len(), 1);
        assert_eq!(plan.cells[0].pos_px, [10.0, 20.0]);
        assert_eq!(plan.cells[0].size_px, [30.0, 40.0]);
        assert_eq!(plan.cells[0].color[0], 128.0 / 255.0);
        assert_eq!(plan.cells[0].color[1], 64.0 / 255.0);
        assert_eq!(plan.cells[0].color[2], 1.0);
        assert_eq!(plan.cells[0].color[3], 127.0 / 255.0);
    }

    #[test]
    fn plan_with_dpr_lowers_logical_paint_ops_to_physical_pixels() {
        let plan = WebGpuBackend::plan_with_dpr(
            &[
                PaintOp::FillRect {
                    rect: Rect {
                        x: 10.0,
                        y: 20.0,
                        w: 30.0,
                        h: 40.0,
                    },
                    color: Color::rgb(255, 255, 255),
                },
                PaintOp::Text {
                    origin: Point { x: 3.0, y: 4.0 },
                    content: "x".to_string(),
                    font: FontSpec {
                        family: "system-ui".to_string(),
                        size_px: 14.0,
                        weight: 400,
                    },
                    color: Color::rgb(0, 0, 0),
                },
            ],
            2.0,
        );

        assert_eq!(plan.cells[0].pos_px, [20.0, 40.0]);
        assert_eq!(plan.cells[0].size_px, [60.0, 80.0]);
        assert_eq!(plan.text_runs[0].origin_px, [6.0, 8.0]);
        assert_eq!(plan.text_runs[0].font_size_px, 28.0);
    }

    #[test]
    fn frame_plan_packs_cells_for_grid_wasm_bridge() {
        let plan = WebGpuFramePlan {
            cells: vec![
                CellInstance {
                    pos_px: [1.0, 2.0],
                    size_px: [3.0, 4.0],
                    color: [0.1, 0.2, 0.3, 0.4],
                },
                CellInstance {
                    pos_px: [5.0, 6.0],
                    size_px: [7.0, 8.0],
                    color: [0.5, 0.6, 0.7, 0.8],
                },
            ],
            text_runs: Vec::new(),
            unsupported: Vec::new(),
        };

        assert_eq!(plan.packed_f32_len(), 16);
        assert_eq!(
            plan.to_packed_f32(),
            vec![1.0, 2.0, 3.0, 4.0, 0.1, 0.2, 0.3, 0.4, 5.0, 6.0, 7.0, 8.0, 0.5, 0.6, 0.7, 0.8]
        );
    }

    #[test]
    fn unsupported_ops_are_reported_without_blocking_rects() {
        let plan = WebGpuBackend::plan(&[
            PaintOp::Text {
                origin: Point { x: 0.0, y: 10.0 },
                content: "count".to_string(),
                font: FontSpec {
                    family: "system-ui".to_string(),
                    size_px: 14.0,
                    weight: 400,
                },
                color: Color::rgb(0, 0, 0),
            },
            PaintOp::FillRect {
                rect: Rect {
                    x: 1.0,
                    y: 2.0,
                    w: 3.0,
                    h: 4.0,
                },
                color: Color::rgb(255, 0, 0),
            },
            PaintOp::PushClip {
                rect: Rect {
                    x: 0.0,
                    y: 0.0,
                    w: 5.0,
                    h: 5.0,
                },
            },
            PaintOp::PopClip,
        ]);

        assert_eq!(plan.cells.len(), 1);
        assert_eq!(plan.text_runs.len(), 1);
        assert_eq!(plan.text_runs[0].origin_px, [0.0, 10.0]);
        assert_eq!(plan.text_runs[0].content, "count");
        assert_eq!(plan.text_runs[0].font_family, "system-ui");
        assert_eq!(plan.text_runs[0].font_size_px, 14.0);
        assert_eq!(plan.text_runs[0].font_weight, 400);
        assert_eq!(plan.text_runs[0].color, [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(
            plan.unsupported,
            vec![WebGpuUnsupportedOp::Clip, WebGpuUnsupportedOp::Clip]
        );
        assert!(!plan.is_complete());
    }

    #[test]
    fn text_runs_are_supported_webgpu_plan_items() {
        let plan = WebGpuBackend::plan(&[PaintOp::Text {
            origin: Point { x: 0.0, y: 10.0 },
            content: "cell 0".to_string(),
            font: FontSpec {
                family: "system-ui".to_string(),
                size_px: 14.0,
                weight: 400,
            },
            color: Color::rgb(0, 0, 0),
        }]);

        assert_eq!(plan.text_runs.len(), 1);
        assert!(
            plan.unsupported.is_empty(),
            "text is lowered by the wasm glyph atlas bridge"
        );
        assert!(plan.is_complete());
    }

    // ── Slice #2117: StrokeRect lowering ──────────────────────────────────

    fn stroke_op(x: f32, y: f32, w: f32, h: f32, width: f32) -> PaintOp {
        PaintOp::StrokeRect {
            rect: Rect { x, y, w, h },
            color: Color {
                r: 10,
                g: 20,
                b: 30,
                a: 255,
            },
            width,
        }
    }

    #[test]
    fn stroke_rect_lowers_to_four_centered_strips() {
        // S1+S2+S3+S4+S7: standard stroke produces 4 strips, center-aligned,
        // with no corner overlap, color forwarded.
        let plan = WebGpuBackend::plan(&[stroke_op(10.0, 20.0, 100.0, 80.0, 2.0)]);
        assert!(plan.is_complete(), "stroke is no longer unsupported");
        assert_eq!(plan.cells.len(), 4);

        let half = 1.0_f32;
        let expected_rgba = [10.0 / 255.0, 20.0 / 255.0, 30.0 / 255.0, 1.0];

        // Top: full-width band at y - half, height = width.
        assert_eq!(plan.cells[0].pos_px, [10.0 - half, 20.0 - half]);
        assert_eq!(plan.cells[0].size_px, [100.0 + 2.0, 2.0]);
        // Bottom: same at y + h - half.
        assert_eq!(plan.cells[1].pos_px, [10.0 - half, 20.0 + 80.0 - half]);
        assert_eq!(plan.cells[1].size_px, [100.0 + 2.0, 2.0]);
        // Left: middle only, at x - half, y + half, height = h - width.
        assert_eq!(plan.cells[2].pos_px, [10.0 - half, 20.0 + half]);
        assert_eq!(plan.cells[2].size_px, [2.0, 80.0 - 2.0]);
        // Right: at x + w - half.
        assert_eq!(plan.cells[3].pos_px, [10.0 + 100.0 - half, 20.0 + half]);
        assert_eq!(plan.cells[3].size_px, [2.0, 80.0 - 2.0]);

        for cell in &plan.cells {
            assert_eq!(cell.color, expected_rgba);
        }
    }

    #[test]
    fn stroke_rect_zero_width_emits_nothing() {
        // S5: width <= 0 -> zero cells, zero unsupported entries.
        let plan = WebGpuBackend::plan(&[stroke_op(0.0, 0.0, 10.0, 10.0, 0.0)]);
        assert!(plan.is_complete());
        assert!(plan.cells.is_empty());
        assert!(plan.unsupported.is_empty());

        let plan = WebGpuBackend::plan(&[stroke_op(0.0, 0.0, 10.0, 10.0, -1.0)]);
        assert!(plan.cells.is_empty());
    }

    #[test]
    fn stroke_rect_zero_size_emits_nothing() {
        // S5: w <= 0 or h <= 0 -> zero cells.
        let plan = WebGpuBackend::plan(&[stroke_op(0.0, 0.0, 0.0, 10.0, 2.0)]);
        assert!(plan.cells.is_empty());

        let plan = WebGpuBackend::plan(&[stroke_op(0.0, 0.0, 10.0, 0.0, 2.0)]);
        assert!(plan.cells.is_empty());
    }

    #[test]
    fn stroke_rect_thick_collapses_left_right() {
        // S6: width >= h -> middle_h <= 0 -> only top + bottom emitted.
        let plan = WebGpuBackend::plan(&[stroke_op(0.0, 0.0, 20.0, 4.0, 4.0)]);
        assert_eq!(plan.cells.len(), 2);
        // Both strips have height = width and span the full horizontal extent.
        for cell in &plan.cells {
            assert_eq!(cell.size_px[0], 20.0 + 4.0);
            assert_eq!(cell.size_px[1], 4.0);
        }
    }

    #[test]
    fn stroke_rect_thick_collapses_top_bottom() {
        // S6 symmetric: width >= w -> top_w + width still > 0 (top spans
        // the full path width even when the rect itself is thinner than
        // the stroke), so top/bottom still emit. Middle vertical only
        // disappears when width >= h, not w. Pin both behaviours so a
        // future refactor doesn't silently drop a strip.
        let plan = WebGpuBackend::plan(&[stroke_op(0.0, 0.0, 2.0, 20.0, 4.0)]);
        // w=2, width=4 -> top_w = 6 > 0; middle_h = 16 > 0 -> 4 cells.
        assert_eq!(plan.cells.len(), 4);
        for cell in &plan.cells[..2] {
            assert_eq!(cell.size_px, [6.0, 4.0]);
        }
        for cell in &plan.cells[2..] {
            assert_eq!(cell.size_px, [4.0, 16.0]);
        }
    }

    #[test]
    fn stroke_rect_no_corner_overlap_with_middle_strips() {
        // S3: left/right top edge starts at y + half (NOT y - half) and
        // ends at y + h - half — strictly inside the top/bottom rows.
        // Pin the y range explicitly so a future "include corners" tweak
        // breaks the test.
        let plan = WebGpuBackend::plan(&[stroke_op(0.0, 0.0, 100.0, 60.0, 6.0)]);
        assert_eq!(plan.cells.len(), 4);
        let half = 3.0_f32;
        // Left strip: y starts at +half, height = 60 - 6 = 54.
        assert_eq!(plan.cells[2].pos_px[1], half);
        assert_eq!(plan.cells[2].size_px[1], 54.0);
        // Top strip ends at y = -half + width = -3 + 6 = 3 == left start.
        let top_bottom_y = plan.cells[0].pos_px[1] + plan.cells[0].size_px[1];
        let left_start_y = plan.cells[2].pos_px[1];
        assert_eq!(
            top_bottom_y, left_start_y,
            "top strip's bottom edge must touch left strip's top edge — no gap, no overlap",
        );
    }

    #[test]
    fn backend_execute_updates_last_frame() {
        let mut backend = WebGpuBackend::new();
        backend.execute(&[PaintOp::FillRect {
            rect: Rect {
                x: 0.0,
                y: 0.0,
                w: 8.0,
                h: 9.0,
            },
            color: Color::rgb(0, 255, 0),
        }]);

        assert_eq!(backend.last_frame().cells.len(), 1);
        assert!(backend.backend_description().contains("PRIMARY"));
    }
}
// CODEGEN-END
