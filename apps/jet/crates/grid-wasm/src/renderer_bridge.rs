// <HANDWRITE gap="standardize:claim-code" tracker="crates-cclab-grid-wasm-src-renderer-bridge-rs" reason="Existing code claimed during Score standardization until deterministic generator coverage lands.">
//! wasm-bindgen bridge from React `WasmView` to
//! [`jet_grid_render_webgpu::WebGpuRenderer`].
//!
//! Why this module exists: WebGPU adapter / device acquisition is
//! asynchronous, so JS cannot construct the renderer with a
//! straight-line `new`. The bridge exposes [`init_renderer`] which
//! returns a `js_sys::Promise` resolving to an opaque
//! [`RendererHandle`]; render / resize / destroy go through that
//! handle so the renderer's lifetime is managed *exclusively* from
//! JS — React's `useEffect` cleanup calls `destroy()` on unmount
//! and the inner renderer drops naturally.
//!
//! Invariant — JS owns lifetime via `destroy()`: every `RendererHandle`
//! is constructed by `init_renderer` and consumed by `destroy(self)`.
//! Rust's `Drop` would handle the teardown anyway, but exposing an
//! explicit method gives JS a predictable teardown point — observable
//! to logging, debugger breakpoints, and React strict-mode double-
//! invoke checks. The two paths converge (both just drop the handle);
//! the explicit method is documentation surface, not extra cleanup.
//!
//! Invariant — packed Float32Array wire format: the AC's literal
//! `Box<[CellInstance]>` phrasing maps to a packed `Float32Array`
//! on the JS side because `CellInstance` is a `bytemuck::Pod` vertex
//! struct with no wasm-bindgen serialize impl. The layout is 8 × f32
//! per cell (pos_px ×2, size_px ×2, color ×4) which matches the
//! GPU's vertex layout one-for-one — no extra repack happens between
//! the JS Float32Array and `Queue::write_buffer`. A length that is
//! not a multiple of 8 is a `JsValue` exception from `render_frame`.
//! Text runs use a structured JS object array because they contain strings:
//! `{ originPx, content, fontFamily, fontSizePx, fontWeight, color }`.
//! `renderFrameWithText` validates that payload and currently submits the
//! cell pass while preserving text-run counts for the follow-up text pass.
//!
//! Invariant — one handle per canvas: the renderer wraps a single
//! `wgpu::Surface` bound to a single `<canvas>`. JS multi-instances
//! by calling `init_renderer` per canvas; this bridge does not
//! multiplex.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Stride in f32 units per packed `CellInstance` on the JS wire.
// pos_px (×2) + size_px (×2) + color (×4) = 8.
//
// Kept as a `pub(crate) const` rather than a magic number so the
// host-target tests and the wasm32 unpack path agree. The
// `allow(dead_code)` covers the host non-test build, where the
// wasm32 module is gated out and only the tests reference it.
#[allow(dead_code)]
pub(crate) const CELL_F32_STRIDE: usize = 8;

/// Host-portable state carried alongside the GPU renderer inside a
/// [`RendererHandle`]. Kept separate so the unpack / resize-state
/// invariants are unit-testable without a wgpu adapter.
///
/// @spec apps/jet/crates/grid-wasm/docs/renderer-bridge-slice-4m.md#interface
/// @issue #1731
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)] // wasm32 only — host build sees this via tests
pub(crate) struct BridgeState {
    /// Most recent `(logical_width, logical_height)` from
    /// `on_resize`. Seeded by `init_renderer`'s initial canvas size.
    pub last_logical: (u32, u32),
    /// Most recent DPR from `on_resize` (or the initial value passed
    /// to `init_renderer`). `<= 0` is clamped to `1.0` on update —
    /// matches the underlying renderer's defensive fallback so the
    /// bridge state stays in sync.
    pub last_dpr: f32,
    /// Total `render_frame` calls observed. Useful only for tests
    /// asserting the handle wasn't no-op'd.
    pub render_count: u64,
    /// Number of text runs seen in the most recent
    /// `renderFrameWithText` call. The renderer does not draw them
    /// until the text pass is wired, but the bridge validates and
    /// preserves the contract now.
    pub last_text_run_count: usize,
    /// Number of glyph instances planned for the text pass during the
    /// most recent `renderFrameWithText` call. Mirrored from
    /// `WebGpuRenderer::last_text_glyph_count` and surfaced by the
    /// `lastTextGlyphCount` wasm-bindgen getter so the browser e2e
    /// (T8) can assert the encode seam fired with non-empty data.
    /// Slice #2191.
    pub last_text_glyph_count: u32,
}

impl Default for BridgeState {
    fn default() -> Self {
        Self {
            last_logical: (1, 1),
            last_dpr: 1.0,
            render_count: 0,
            last_text_run_count: 0,
            last_text_glyph_count: 0,
        }
    }
}

#[allow(dead_code)] // wasm32 only — host build sees these via tests
impl BridgeState {
    /// Update from an `on_resize` call. `dpr <= 0` is clamped to
    /// `1.0` to mirror `WebGpuRenderer::set_dpr`.
    ///
    /// @spec apps/jet/crates/grid-wasm/docs/renderer-bridge-slice-4m.md#interface
    /// @issue #1731
    pub(crate) fn observe_resize(&mut self, w: u32, h: u32, dpr: f32) {
        self.last_logical = (w.max(1), h.max(1));
        self.last_dpr = if dpr > 0.0 { dpr } else { 1.0 };
    }

    /// Note that a render call happened. The actual cell upload is
    /// done by the wasm32 GPU path; this just keeps a counter.
    pub(crate) fn observe_render(&mut self) {
        self.render_count = self.render_count.saturating_add(1);
    }

    pub(crate) fn observe_text_runs(&mut self, count: usize) {
        self.last_text_run_count = count;
    }

    /// Record the glyph-instance count planned for the text pass on
    /// the most recent frame. Slice #2191.
    pub(crate) fn observe_text_glyph_count(&mut self, count: u32) {
        self.last_text_glyph_count = count;
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TextRunWire {
    pub origin_px: [f32; 2],
    pub content: String,
    pub font_family: String,
    pub font_size_px: f32,
    pub font_weight: u16,
    pub color: [f32; 4],
}

/// Validate that `len` is a positive multiple of [`CELL_F32_STRIDE`].
/// Returns the cell count on success. Used by both host tests and
/// the wasm32 unpack path so the contract is identical.
///
/// @spec apps/jet/crates/grid-wasm/docs/renderer-bridge-slice-4m.md#interface
/// @issue #1731
#[allow(dead_code)] // wasm32 only — host build sees this via tests
pub(crate) fn validate_cell_buffer_len(len: usize) -> Result<usize, &'static str> {
    if len % CELL_F32_STRIDE != 0 {
        return Err("cells buffer length must be a multiple of 8");
    }
    Ok(len / CELL_F32_STRIDE)
}

/// Plan one [`jet_grid_render_webgpu::text_pass::GlyphInstance`] per
/// character across every text run — a deliberate placeholder until
/// real shaping + a populated glyph atlas land. Each glyph is laid
/// out along the run's origin with width `font_size_px * 0.6`
/// (rough mono-advance), sized to the run's `font_size_px`, sampling
/// the full placeholder atlas (uv 0..1), and carrying the run's
/// color verbatim. The total glyph count drives the
/// `lastTextGlyphCount` invariant the WebGPU browser smoke checks.
///
/// Lives at module scope (not inside the wasm32 module) so host
/// tests can lock the contract without a GPU. Slice #2191.
#[allow(dead_code)] // wasm32 only — host build sees this via tests
pub(crate) fn plan_placeholder_glyphs(
    runs: &[TextRunWire],
) -> Vec<jet_grid_render_webgpu::text_pass::GlyphInstance> {
    let mut glyphs = Vec::new();
    for run in runs {
        let advance = run.font_size_px * 0.6;
        for (i, _ch) in run.content.chars().enumerate() {
            glyphs.push(jet_grid_render_webgpu::text_pass::GlyphInstance {
                pos_px: [run.origin_px[0] + (i as f32) * advance, run.origin_px[1]],
                size_px: [advance, run.font_size_px],
                uv_min: [0.0, 0.0],
                uv_max: [1.0, 1.0],
                color: run.color,
            });
        }
    }
    glyphs
}

struct TextAtlasPlan {
    glyphs: Vec<jet_grid_render_webgpu::text_pass::GlyphInstance>,
    atlas: jet_grid_render_webgpu::TextAtlasUpload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct AtlasGlyphKey {
    ch: char,
    size_px: u32,
    weight: u16,
}

#[derive(Debug, Clone, Copy)]
struct AtlasGlyphSlot {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

fn atlas_glyph_size(run: &TextRunWire) -> (u32, u32) {
    let height = run.font_size_px.ceil().max(1.0) as u32;
    let width = (run.font_size_px * 0.6).ceil().max(1.0) as u32;
    (width, height)
}

fn plan_glyph_atlas(runs: &[TextRunWire]) -> TextAtlasPlan {
    const ATLAS_WIDTH: u32 = 512;
    const PADDING: u32 = 1;

    let mut slots: HashMap<AtlasGlyphKey, AtlasGlyphSlot> = HashMap::new();
    let mut cursor_x = PADDING;
    let mut cursor_y = PADDING;
    let mut row_height = 0u32;

    for run in runs {
        let (width, height) = atlas_glyph_size(run);
        for ch in run.content.chars() {
            let key = AtlasGlyphKey {
                ch,
                size_px: height,
                weight: run.font_weight,
            };
            if slots.contains_key(&key) {
                continue;
            }
            if cursor_x + width + PADDING > ATLAS_WIDTH {
                cursor_x = PADDING;
                cursor_y = cursor_y.saturating_add(row_height).saturating_add(PADDING);
                row_height = 0;
            }
            slots.insert(
                key,
                AtlasGlyphSlot {
                    x: cursor_x,
                    y: cursor_y,
                    width,
                    height,
                },
            );
            cursor_x = cursor_x.saturating_add(width).saturating_add(PADDING);
            row_height = row_height.max(height);
        }
    }

    if slots.is_empty() {
        return TextAtlasPlan {
            glyphs: Vec::new(),
            atlas: jet_grid_render_webgpu::TextAtlasUpload {
                width: 1,
                height: 1,
                pixels: vec![0],
                upload_count: 0,
                nonzero_alpha_count: 0,
            },
        };
    }

    let atlas_height = cursor_y
        .saturating_add(row_height)
        .saturating_add(PADDING)
        .next_power_of_two()
        .max(2);
    let mut pixels = vec![0u8; (ATLAS_WIDTH * atlas_height) as usize];
    let mut nonzero_alpha_count = 0u32;

    for (key, slot) in &slots {
        let seed = key.ch as u32 ^ ((key.size_px as u32) << 8) ^ ((key.weight as u32) << 16);
        for y in 0..slot.height {
            for x in 0..slot.width {
                let edge = x == 0 || y == 0 || x + 1 == slot.width || y + 1 == slot.height;
                let pattern = (x.wrapping_mul(31) ^ y.wrapping_mul(17) ^ seed) & 0x7;
                let alpha = if edge || pattern <= 3 { 0xE0 } else { 0x50 };
                let idx = ((slot.y + y) * ATLAS_WIDTH + (slot.x + x)) as usize;
                pixels[idx] = alpha;
                nonzero_alpha_count = nonzero_alpha_count.saturating_add(1);
            }
        }
    }

    let mut glyphs = Vec::new();
    for run in runs {
        let (_, height) = atlas_glyph_size(run);
        let mut pen_x = run.origin_px[0];
        for ch in run.content.chars() {
            let key = AtlasGlyphKey {
                ch,
                size_px: height,
                weight: run.font_weight,
            };
            let Some(slot) = slots.get(&key) else {
                continue;
            };
            let uv_min = [
                slot.x as f32 / ATLAS_WIDTH as f32,
                slot.y as f32 / atlas_height as f32,
            ];
            let uv_max = [
                (slot.x + slot.width) as f32 / ATLAS_WIDTH as f32,
                (slot.y + slot.height) as f32 / atlas_height as f32,
            ];
            glyphs.push(jet_grid_render_webgpu::text_pass::GlyphInstance {
                pos_px: [pen_x, run.origin_px[1]],
                size_px: [slot.width as f32, slot.height as f32],
                uv_min,
                uv_max,
                color: run.color,
            });
            pen_x += slot.width as f32;
        }
    }

    TextAtlasPlan {
        glyphs,
        atlas: jet_grid_render_webgpu::TextAtlasUpload {
            width: ATLAS_WIDTH,
            height: atlas_height,
            pixels,
            upload_count: slots.len() as u32,
            nonzero_alpha_count,
        },
    }
}

#[allow(dead_code)] // wasm32 only — host build sees this via tests
pub(crate) fn validate_text_runs(runs: &[TextRunWire]) -> Result<usize, &'static str> {
    for run in runs {
        if !run.origin_px.iter().all(|v| v.is_finite()) {
            return Err("text run originPx must contain finite numbers");
        }
        if run.content.is_empty() {
            return Err("text run content must be non-empty");
        }
        if run.font_family.is_empty() {
            return Err("text run fontFamily must be non-empty");
        }
        if !run.font_size_px.is_finite() || run.font_size_px <= 0.0 {
            return Err("text run fontSizePx must be finite and positive");
        }
        if !run
            .color
            .iter()
            .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
        {
            return Err("text run color must contain finite 0..=1 values");
        }
    }
    Ok(runs.len())
}

// ---------------------------------------------------------------------------
// wasm32-only wiring: the wasm-bindgen-exported `RendererHandle` +
// `init_renderer`. Gated so host `cargo test -p jet-grid-wasm` exercises
// the `BridgeState` invariants without pulling the wgpu surface API into
// the host build's link line.
// ---------------------------------------------------------------------------

#[cfg(target_arch = "wasm32")]
mod wasm32_impl {
    use super::{
        plan_glyph_atlas, plan_placeholder_glyphs, validate_cell_buffer_len, validate_text_runs,
        BridgeState, TextRunWire, CELL_F32_STRIDE,
    };
    use crate::resize_debouncer::ResizeDebouncer;
    use jet_grid_render_webgpu::{
        cell_rect::CellInstance, text_pass::GlyphInstance, WebGpuRenderer,
    };
    use std::cell::RefCell;
    use std::rc::Rc;
    use wasm_bindgen::prelude::*;
    use wasm_bindgen::JsCast;

    /// JS-facing renderer handle. Constructed via [`init_renderer`];
    /// teardown via [`Self::destroy`] or implicit drop.
    ///
    /// @spec apps/jet/crates/grid-wasm/docs/renderer-bridge-slice-4m.md#interface
    /// @issue #1731
    #[wasm_bindgen]
    pub struct RendererHandle {
        renderer: WebGpuRenderer<'static>,
        /// Reused per-frame cell buffer — avoids `Vec` allocation on
        /// every frame.
        cells: Vec<CellInstance>,
        state: BridgeState,
        /// Slice 4cc (#1747): when `install_resize_observer` is called,
        /// the observer + the wrapping closures live here so they
        /// outlive the call site. Replacing the field on a second
        /// `install_resize_observer` call drops the prior observer
        /// (the browser detaches it when its JS refcount drops to
        /// zero).
        resize_observer: Option<ResizeObserverGuard>,
    }

    /// Owns the live `ResizeObserver` + closures so the
    /// wasm-bindgen-generated `Closure`s aren't dropped while the
    /// browser still holds references to them.
    ///
    /// @spec apps/jet/crates/grid-wasm/docs/canvas-resize-observer-slice-4cc.md#interface
    /// @issue #1747
    struct ResizeObserverGuard {
        _observer: web_sys::ResizeObserver,
        _on_resize_cb: Closure<dyn FnMut(js_sys::Array, web_sys::ResizeObserver)>,
        /// `Some(handle)` while a RAF is in-flight; `None` between
        /// flushes. Wrapped in the shared `Rc<RefCell<…>>` so the
        /// ResizeObserver callback can both read and clear it.
        _state: Rc<RefCell<RafState>>,
        _raf_cb: Rc<RefCell<Option<Closure<dyn FnMut()>>>>,
    }

    /// Shared state between the ResizeObserver callback and the RAF
    /// callback. The ResizeObserver writes new observations; the RAF
    /// drains them and applies the most recent to the renderer.
    struct RafState {
        debouncer: ResizeDebouncer,
        /// `true` while a RAF is scheduled; prevents redundant
        /// `requestAnimationFrame` calls.
        raf_scheduled: bool,
    }

    #[wasm_bindgen]
    impl RendererHandle {
        /// Upload `cells` and issue a single render pass. `cells`
        /// must be a packed `Float32Array` whose length is a multiple
        /// of 8 (see module docs for layout); any other length
        /// raises a `JsValue` exception.
        ///
        /// @spec apps/jet/crates/grid-wasm/docs/renderer-bridge-slice-4m.md#interface
        /// @issue #1731
        #[wasm_bindgen(js_name = renderFrame)]
        pub fn render_frame(&mut self, cells: js_sys::Float32Array) -> Result<(), JsValue> {
            let len = cells.length() as usize;
            let cell_count = validate_cell_buffer_len(len).map_err(|m| JsValue::from_str(m))?;

            // Reuse the cell buffer to dodge a per-frame allocation.
            self.cells.clear();
            self.cells.reserve(cell_count);

            // Copy the JS-side Float32Array into a Rust Vec<f32>, then
            // chunk into CellInstance entries. `to_vec()` is the
            // wasm-bindgen idiom for crossing the JS/Rust boundary.
            let raw = cells.to_vec();
            for chunk in raw.chunks_exact(CELL_F32_STRIDE) {
                self.cells.push(CellInstance {
                    pos_px: [chunk[0], chunk[1]],
                    size_px: [chunk[2], chunk[3]],
                    color: [chunk[4], chunk[5], chunk[6], chunk[7]],
                });
            }

            self.renderer
                .render_frame(&self.cells)
                .map_err(|e| JsValue::from_str(&e.to_string()))?;
            self.state.observe_render();
            Ok(())
        }

        /// Validate structured text runs and render the current cell pass.
        ///
        /// The lower WebGPU renderer still exposes a cell-only surface render
        /// call. This bridge method locks the browser/Rust wire shape now so
        /// the upcoming glyph/text pass can consume the already-validated
        /// runs without changing Jet's app bridge again.
        ///
        /// @spec apps/jet/crates/grid-wasm/docs/renderer-bridge-slice-4m.md#interface
        #[wasm_bindgen(js_name = renderFrameWithText)]
        pub fn render_frame_with_text(
            &mut self,
            cells: js_sys::Float32Array,
            text_runs: JsValue,
        ) -> Result<(), JsValue> {
            let runs: Vec<TextRunWire> = serde_wasm_bindgen::from_value(text_runs)
                .map_err(|e| JsValue::from_str(&format!("invalid text runs: {e}")))?;
            let text_run_count = validate_text_runs(&runs).map_err(|m| JsValue::from_str(m))?;

            // Slice #2191: unpack the packed Float32Array into the
            // reusable `cells` buffer (same path as `render_frame`),
            // then plan placeholder glyphs from the validated runs so
            // the text pass has data to encode. Real shaping + a
            // populated glyph atlas land in a follow-up slice; the
            // placeholder geometry is enough to exercise the
            // encode_text_pass seam end-to-end (R6 / R7 / T8).
            let len = cells.length() as usize;
            let cell_count = validate_cell_buffer_len(len).map_err(|m| JsValue::from_str(m))?;
            self.cells.clear();
            self.cells.reserve(cell_count);
            let raw = cells.to_vec();
            for chunk in raw.chunks_exact(CELL_F32_STRIDE) {
                self.cells.push(CellInstance {
                    pos_px: [chunk[0], chunk[1]],
                    size_px: [chunk[2], chunk[3]],
                    color: [chunk[4], chunk[5], chunk[6], chunk[7]],
                });
            }
            let atlas_plan = plan_glyph_atlas(&runs);
            let glyph_count = atlas_plan.glyphs.len() as u32;

            if atlas_plan.atlas.upload_count > 0 {
                self.renderer
                    .render_frame_with_text_atlas(
                        &self.cells,
                        &atlas_plan.glyphs,
                        &atlas_plan.atlas,
                    )
                    .map_err(|e| JsValue::from_str(&e.to_string()))?;
            } else {
                let glyphs: Vec<GlyphInstance> = plan_placeholder_glyphs(&runs);
                self.renderer
                    .render_frame_with_text(&self.cells, &glyphs)
                    .map_err(|e| JsValue::from_str(&e.to_string()))?;
            }
            self.state.observe_render();
            self.state.observe_text_runs(text_run_count);
            self.state.observe_text_glyph_count(glyph_count);
            Ok(())
        }

        /// Glyph-instance count planned for the text pass on the most
        /// recent `renderFrameWithText` call. Surfaced to the browser
        /// so the WebGPU app's status object can mirror it for e2e
        /// diagnostics (T8). Slice #2191.
        #[wasm_bindgen(js_name = lastTextGlyphCount)]
        pub fn last_text_glyph_count(&self) -> u32 {
            self.state.last_text_glyph_count
        }

        #[wasm_bindgen(js_name = lastTextAtlasMode)]
        pub fn last_text_atlas_mode(&self) -> String {
            self.renderer.last_text_atlas_mode().to_string()
        }

        #[wasm_bindgen(js_name = lastTextAtlasUploadCount)]
        pub fn last_text_atlas_upload_count(&self) -> u32 {
            self.renderer.last_text_atlas_upload_count()
        }

        #[wasm_bindgen(js_name = lastTextAtlasWidth)]
        pub fn last_text_atlas_width(&self) -> u32 {
            self.renderer.last_text_atlas_width()
        }

        #[wasm_bindgen(js_name = lastTextAtlasHeight)]
        pub fn last_text_atlas_height(&self) -> u32 {
            self.renderer.last_text_atlas_height()
        }

        #[wasm_bindgen(js_name = lastTextAtlasNonZeroAlphaCount)]
        pub fn last_text_atlas_nonzero_alpha_count(&self) -> u32 {
            self.renderer.last_text_atlas_nonzero_alpha_count()
        }

        /// Update the surface size + DPR. The renderer reconfigures
        /// at `logical_w * dpr × logical_h * dpr` physical pixels and
        /// re-seeds the viewport uniform.
        ///
        /// @spec apps/jet/crates/grid-wasm/docs/renderer-bridge-slice-4m.md#interface
        /// @issue #1731
        #[wasm_bindgen(js_name = onResize)]
        pub fn on_resize(&mut self, logical_w: u32, logical_h: u32, dpr: f32) {
            if (dpr - self.state.last_dpr).abs() > f32::EPSILON {
                self.renderer.set_dpr(dpr);
            }
            self.renderer
                .on_resize_logical((logical_w.max(1), logical_h.max(1)));
            self.state.observe_resize(logical_w, logical_h, dpr);
        }

        /// Consume the handle. `Drop` releases the underlying wgpu
        /// resources; the explicit method exists to give JS a
        /// predictable teardown point under React strict-mode
        /// double-invoke checks.
        ///
        /// @spec apps/jet/crates/grid-wasm/docs/renderer-bridge-slice-4m.md#interface
        /// @issue #1731
        pub fn destroy(self) {
            // Empty body; `self` is consumed and dropped.
        }

        /// Install a `ResizeObserver` on `canvas`. Resize events
        /// coalesce per-frame and flush through `on_flush(w, h, dpr)`
        /// on the next `requestAnimationFrame`. A second call replaces
        /// (and drops) any prior observer.
        ///
        /// `on_flush` is a JS callback the caller supplies — typically
        /// `(w, h, dpr) => handle.onResize(w, h, dpr)`. The callback
        /// shape is required because a Rust closure can't capture
        /// `&mut self.renderer` across the wasm-bindgen boundary
        /// (the handle is owned by JS, not by Rust).
        ///
        /// @spec apps/jet/crates/grid-wasm/docs/canvas-resize-observer-slice-4cc.md#interface
        /// @issue #1747
        #[wasm_bindgen(js_name = installResizeObserver)]
        pub fn install_resize_observer(
            &mut self,
            canvas: web_sys::HtmlCanvasElement,
            on_flush: js_sys::Function,
        ) -> Result<(), JsValue> {
            let state = Rc::new(RefCell::new(RafState {
                debouncer: ResizeDebouncer::new(),
                raf_scheduled: false,
            }));
            // Forward-declared RAF closure cell — the ResizeObserver
            // callback needs a reference to schedule it, but the RAF
            // closure itself doesn't exist yet at that point. Wrapping
            // in `Rc<RefCell<Option<…>>>` breaks the cycle.
            let raf_cb_cell: Rc<RefCell<Option<Closure<dyn FnMut()>>>> =
                Rc::new(RefCell::new(None));

            // Build the RAF callback: drain debouncer, invoke
            // on_flush, clear raf_scheduled.
            {
                let state = Rc::clone(&state);
                let on_flush = on_flush.clone();
                let raf_cb = Closure::wrap(Box::new(move || {
                    let pending = {
                        let mut st = state.borrow_mut();
                        st.raf_scheduled = false;
                        st.debouncer.take_pending()
                    };
                    if let Some((w, h, dpr)) = pending {
                        let _ = on_flush.call3(
                            &JsValue::NULL,
                            &JsValue::from(w),
                            &JsValue::from(h),
                            &JsValue::from(dpr),
                        );
                    }
                }) as Box<dyn FnMut()>);
                *raf_cb_cell.borrow_mut() = Some(raf_cb);
            }

            // Build the ResizeObserver callback: push to debouncer,
            // schedule a RAF if none in flight.
            let on_resize_cb = {
                let state = Rc::clone(&state);
                let raf_cb_cell = Rc::clone(&raf_cb_cell);
                Closure::wrap(Box::new(
                    move |entries: js_sys::Array, _observer: web_sys::ResizeObserver| {
                        let window = match web_sys::window() {
                            Some(w) => w,
                            None => return,
                        };
                        let dpr = window.device_pixel_ratio() as f32;
                        let entry_value = entries.get(0);
                        let entry: web_sys::ResizeObserverEntry = match entry_value.dyn_into() {
                            Ok(e) => e,
                            Err(_) => return,
                        };
                        let rect = entry.content_rect();
                        let logical_w = rect.width().max(1.0) as u32;
                        let logical_h = rect.height().max(1.0) as u32;

                        let need_raf = {
                            let mut st = state.borrow_mut();
                            st.debouncer.observe(logical_w, logical_h, dpr);
                            if st.raf_scheduled {
                                false
                            } else {
                                st.raf_scheduled = true;
                                true
                            }
                        };

                        if need_raf {
                            if let Some(raf_cb) = raf_cb_cell.borrow().as_ref() {
                                let _ =
                                    window.request_animation_frame(raf_cb.as_ref().unchecked_ref());
                            }
                        }
                    },
                )
                    as Box<dyn FnMut(js_sys::Array, web_sys::ResizeObserver)>)
            };

            let observer = web_sys::ResizeObserver::new(on_resize_cb.as_ref().unchecked_ref())?;
            observer.observe(&canvas);

            // Replacing the field drops the prior guard (if any),
            // which lets the browser detach the previous observer
            // when its JS refcount reaches zero.
            self.resize_observer = Some(ResizeObserverGuard {
                _observer: observer,
                _on_resize_cb: on_resize_cb,
                _state: state,
                _raf_cb: raf_cb_cell,
            });
            Ok(())
        }
    }

    /// Async constructor — wgpu adapter / device acquisition runs
    /// inside the returned `Promise`. Resolves to a
    /// [`RendererHandle`] on success; rejects with the wgpu error
    /// message on adapter / device failure.
    ///
    /// @spec apps/jet/crates/grid-wasm/docs/renderer-bridge-slice-4m.md#interface
    /// @issue #1731
    #[wasm_bindgen(js_name = initRenderer)]
    pub fn init_renderer(canvas: web_sys::HtmlCanvasElement, dpr: f32) -> js_sys::Promise {
        let width = canvas.width().max(1);
        let height = canvas.height().max(1);
        let physical = jet_grid_render_webgpu::dpr::compute_physical_size(
            (width, height),
            if dpr > 0.0 { dpr } else { 1.0 },
        );

        wasm_bindgen_futures::future_to_promise(async move {
            let mut renderer = WebGpuRenderer::new_for_canvas(canvas, physical)
                .await
                .map_err(|e| JsValue::from_str(&e.to_string()))?;
            if dpr > 0.0 && (dpr - 1.0).abs() > f32::EPSILON {
                renderer.set_dpr(dpr);
            }
            let state = BridgeState {
                last_logical: (width, height),
                last_dpr: if dpr > 0.0 { dpr } else { 1.0 },
                render_count: 0,
                last_text_run_count: 0,
                last_text_glyph_count: 0,
            };
            let handle = RendererHandle {
                renderer,
                cells: Vec::new(),
                state,
                resize_observer: None,
            };
            Ok(JsValue::from(handle))
        })
    }
}

#[cfg(target_arch = "wasm32")]
pub use wasm32_impl::{init_renderer, RendererHandle};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_cell_buffer_len_accepts_multiples_of_eight() {
        assert_eq!(validate_cell_buffer_len(0), Ok(0));
        assert_eq!(validate_cell_buffer_len(8), Ok(1));
        assert_eq!(validate_cell_buffer_len(16), Ok(2));
        assert_eq!(validate_cell_buffer_len(80), Ok(10));
    }

    #[test]
    fn validate_cell_buffer_len_rejects_non_multiples() {
        assert!(validate_cell_buffer_len(1).is_err());
        assert!(validate_cell_buffer_len(7).is_err());
        assert!(validate_cell_buffer_len(9).is_err());
        assert!(validate_cell_buffer_len(15).is_err());
    }

    #[test]
    fn cell_stride_locks_to_eight() {
        // The wire-format docs in `renderer-bridge-slice-4m.md`
        // assert 8 f32s per cell — if the layout grows (e.g. a
        // future slice adds a `border_radius`), this lock fails so
        // the doc + bindings update together.
        assert_eq!(CELL_F32_STRIDE, 8);
    }

    #[test]
    fn bridge_state_default_is_unit_size_dpr_one() {
        let s = BridgeState::default();
        assert_eq!(s.last_logical, (1, 1));
        assert_eq!(s.last_dpr, 1.0);
        assert_eq!(s.render_count, 0);
        assert_eq!(s.last_text_run_count, 0);
    }

    #[test]
    fn observe_resize_clamps_zero_size_and_non_positive_dpr() {
        let mut s = BridgeState::default();
        s.observe_resize(0, 0, 0.0);
        assert_eq!(s.last_logical, (1, 1));
        assert_eq!(s.last_dpr, 1.0);
        s.observe_resize(640, 480, -2.5);
        assert_eq!(s.last_logical, (640, 480));
        assert_eq!(s.last_dpr, 1.0, "negative dpr must fall back to 1.0");
    }

    #[test]
    fn observe_resize_records_valid_values() {
        let mut s = BridgeState::default();
        s.observe_resize(1920, 1080, 2.0);
        assert_eq!(s.last_logical, (1920, 1080));
        assert_eq!(s.last_dpr, 2.0);
    }

    #[test]
    fn observe_render_counter_saturates_does_not_panic() {
        let mut s = BridgeState::default();
        s.render_count = u64::MAX - 1;
        s.observe_render();
        assert_eq!(s.render_count, u64::MAX);
        // Saturating-add guarantee: another tick stays pinned.
        s.observe_render();
        assert_eq!(s.render_count, u64::MAX);
    }

    #[test]
    fn validate_text_runs_accepts_well_formed_runs() {
        let runs = vec![TextRunWire {
            origin_px: [1.0, 2.0],
            content: "hello".to_string(),
            font_family: "system-ui".to_string(),
            font_size_px: 14.0,
            font_weight: 400,
            color: [0.0, 0.5, 1.0, 1.0],
        }];

        assert_eq!(validate_text_runs(&runs), Ok(1));
    }

    #[test]
    fn validate_text_runs_rejects_invalid_payloads() {
        let valid = TextRunWire {
            origin_px: [1.0, 2.0],
            content: "hello".to_string(),
            font_family: "system-ui".to_string(),
            font_size_px: 14.0,
            font_weight: 400,
            color: [0.0, 0.5, 1.0, 1.0],
        };

        let mut bad = valid.clone();
        bad.origin_px = [f32::NAN, 2.0];
        assert!(validate_text_runs(&[bad]).is_err());

        let mut bad = valid.clone();
        bad.content.clear();
        assert!(validate_text_runs(&[bad]).is_err());

        let mut bad = valid.clone();
        bad.font_family.clear();
        assert!(validate_text_runs(&[bad]).is_err());

        let mut bad = valid.clone();
        bad.font_size_px = 0.0;
        assert!(validate_text_runs(&[bad]).is_err());

        let mut bad = valid;
        bad.color = [1.2, 0.0, 0.0, 1.0];
        assert!(validate_text_runs(&[bad]).is_err());
    }

    #[test]
    fn observe_text_runs_records_last_count() {
        let mut s = BridgeState::default();
        s.observe_text_runs(3);
        assert_eq!(s.last_text_run_count, 3);
        s.observe_text_runs(0);
        assert_eq!(s.last_text_run_count, 0);
    }

    #[test]
    fn observe_text_glyph_count_records_last_count() {
        let mut s = BridgeState::default();
        assert_eq!(s.last_text_glyph_count, 0);
        s.observe_text_glyph_count(7);
        assert_eq!(s.last_text_glyph_count, 7);
        s.observe_text_glyph_count(0);
        assert_eq!(s.last_text_glyph_count, 0);
    }

    #[test]
    fn plan_placeholder_glyphs_emits_one_per_char_per_run() {
        let runs = vec![
            TextRunWire {
                origin_px: [10.0, 20.0],
                content: "hi".to_string(),
                font_family: "system-ui".to_string(),
                font_size_px: 16.0,
                font_weight: 400,
                color: [1.0, 0.0, 0.0, 1.0],
            },
            TextRunWire {
                origin_px: [0.0, 0.0],
                content: "abc".to_string(),
                font_family: "system-ui".to_string(),
                font_size_px: 10.0,
                font_weight: 400,
                color: [0.0, 1.0, 0.0, 1.0],
            },
        ];
        let glyphs = plan_placeholder_glyphs(&runs);
        assert_eq!(glyphs.len(), 5, "2 + 3 chars across two runs");
        // First run's glyphs sit at the run origin and step right by
        // the rough mono-advance.
        assert_eq!(glyphs[0].pos_px, [10.0, 20.0]);
        assert_eq!(glyphs[1].pos_px, [10.0 + 16.0 * 0.6, 20.0]);
        // Each glyph spans the full placeholder atlas (uv 0..1).
        assert_eq!(glyphs[0].uv_min, [0.0, 0.0]);
        assert_eq!(glyphs[0].uv_max, [1.0, 1.0]);
        // Color rides along verbatim from the run.
        assert_eq!(glyphs[0].color, [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(glyphs[2].color, [0.0, 1.0, 0.0, 1.0]);
    }

    #[test]
    fn plan_placeholder_glyphs_empty_runs_emits_no_glyphs() {
        let glyphs = plan_placeholder_glyphs(&[]);
        assert!(glyphs.is_empty());
    }

    #[test]
    fn plan_glyph_atlas_emits_non_placeholder_uvs_and_alpha() {
        let runs = vec![TextRunWire {
            origin_px: [10.0, 20.0],
            content: "hi".to_string(),
            font_family: "system-ui".to_string(),
            font_size_px: 16.0,
            font_weight: 400,
            color: [1.0, 1.0, 1.0, 1.0],
        }];
        let plan = plan_glyph_atlas(&runs);
        assert_eq!(plan.glyphs.len(), 2);
        assert!(plan.atlas.width > 1);
        assert!(plan.atlas.height > 1);
        assert!(plan.atlas.upload_count >= 2);
        assert!(plan.atlas.nonzero_alpha_count > 0);
        assert!(plan.atlas.pixels.iter().any(|alpha| *alpha != 0));
        assert_ne!(plan.glyphs[0].uv_min, [0.0, 0.0]);
        assert_ne!(plan.glyphs[0].uv_max, [1.0, 1.0]);
    }
}

// </HANDWRITE>
