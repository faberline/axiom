// SPEC-MANAGED: .aw/tech-design/projects/jet/semantic/jet-wasm-src-react.md#schema
// CODEGEN-BEGIN
//! WebGPU event loop — wires a mounted React component to the shared
//! `jet-grid-wasm` renderer handle.
//!
//! React produces an `Element` tree, Jet lays it out and emits `PaintOp`s,
//! then `WebGpuBackend` lowers
//! rect fills into the packed `Float32Array` consumed by
//! `RendererHandle.renderFrame`.

#![cfg(all(feature = "webgpu-app", target_arch = "wasm32"))]

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

use crate::renderer::{
    self,
    selection::{hit_test_visible_cell, visible_table_cells, CellSelectionState},
    LayoutTree, PaintBackend, PaintOp, Point, Renderer, ScrollOffsets, Theme, Viewport,
    WebGpuBackend,
};
use crate::Component;

use super::{mount, set_update_scheduler, MountHandle};

#[cfg(feature = "debug")]
use crate::debug::{DebugBridgeState, JetDebug};

/// JS-owned WebGPU app handle. Dropping the Rust-side handle would release
/// the JS value, but an explicit `destroy()` gives browser callers the same
/// lifecycle shape as `jet-grid-wasm::RendererHandle`.
/// @spec .aw/tech-design/projects/jet/semantic/jet-wasm-src-react.md#schema
#[wasm_bindgen]
pub struct JetWebGpuApp {
    grid_handle: JsValue,
    status: js_sys::Object,
}

/// @spec .aw/tech-design/projects/jet/semantic/jet-wasm-src-react.md#schema
#[wasm_bindgen]
impl JetWebGpuApp {
    pub fn destroy(self) -> Result<(), JsValue> {
        set_update_scheduler(None);
        set_status_str(&self.status, "phase", "destroyed");
        call_method0(&self.grid_handle, "destroy")
    }

    pub fn status(&self) -> JsValue {
        self.status.clone().into()
    }
}

/// Mount `component` on `#<canvas_id>` and initialize the shared WebGPU
/// renderer asynchronously. The returned Promise resolves to `JetWebGpuApp`
/// after the adapter/device is ready and the first frame has rendered.
/// @spec .aw/tech-design/projects/jet/semantic/jet-wasm-src-react.md#schema
pub fn run(canvas_id: &str, component: Component) -> Result<js_sys::Promise, JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
    let document = window
        .document()
        .ok_or_else(|| JsValue::from_str("no document"))?;
    let canvas = document
        .get_element_by_id(canvas_id)
        .ok_or_else(|| JsValue::from_str("canvas element not found"))?
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .map_err(|_| JsValue::from_str("element is not an HTMLCanvasElement"))?;

    let dpr = window.device_pixel_ratio() as f32;
    let css_w = canvas.client_width().max(1) as f32;
    let css_h = canvas.client_height().max(1) as f32;
    canvas.set_width((css_w * dpr) as u32);
    canvas.set_height((css_h * dpr) as u32);
    let status = init_status(&window);
    set_status_str(&status, "phase", "initializing");
    set_status_num(&status, "frames", 0.0);
    set_status_num(&status, "lastCellCount", 0.0);
    set_status_num(&status, "lastTextRunCount", 0.0);
    set_status_num(&status, "lastTextGlyphCount", 0.0);
    set_status_num(&status, "lastUnsupportedCount", 0.0);
    set_status_num(&status, "lastTextAtlasUploadCount", 0.0);
    set_status_num(&status, "lastTextAtlasWidth", 0.0);
    set_status_num(&status, "lastTextAtlasHeight", 0.0);
    set_status_num(&status, "lastTextAtlasNonZeroAlphaCount", 0.0);
    set_status_bool(&status, "lastTextPlanCacheHit", false);
    set_status_num(&status, "textPlanCacheHits", 0.0);
    set_status_num(&status, "textPlanCacheMisses", 0.0);
    set_status_bool(&status, "gpuTimingEnabled", false);
    set_status_bool(&status, "gpuTimingSampleReady", false);
    set_status_value(&status, "lastFrameGpuMs", &JsValue::NULL);
    set_status_num(&status, "lastPaintOpCount", 0.0);
    set_status_num(&status, "lastRepaintCpuMs", 0.0);
    set_status_num(&status, "repaintRequests", 0.0);
    set_status_num(&status, "scheduledRepaintRequests", 0.0);
    set_status_num(&status, "coalescedRepaintRequests", 0.0);
    set_status_bool(&status, "scheduledRepaintPending", false);
    set_status_str(&status, "lastRepaintReason", "");
    set_status_str(&status, "lastRepaintRequestReason", "");
    set_status_num(&status, "lastSelectionCellCount", 0.0);
    set_status_num(&status, "copyCount", 0.0);
    set_status_num(&status, "clipboardWriteSuccessCount", 0.0);
    set_status_num(&status, "clipboardWriteErrorCount", 0.0);
    set_status_bool(&status, "selectionActive", false);
    set_status_str(&status, "selectionRange", "");
    set_status_str(&status, "lastCopiedTsv", "");
    set_status_str(&status, "clipboardWriteState", "idle");
    set_status_str(&status, "clipboardWriteError", "");
    set_status_num(&status, "scrollLeft", 0.0);
    set_status_num(&status, "scrollTop", 0.0);
    set_status_num(&status, "scrollMaxLeft", 0.0);
    set_status_num(&status, "scrollMaxTop", 0.0);
    set_status_bool(&status, "scrollbarVisible", false);
    set_status_str(&status, "bridgeMode", "uninitialized");
    set_status_str(&status, "textAtlasMode", "placeholder");

    let renderer = Rc::new(RefCell::new(Renderer::new(
        Viewport {
            width: css_w,
            height: css_h,
            dpr,
        },
        Theme::default(),
        WebGpuBackend::new(),
    )));
    let handle = Rc::new(mount(component));
    let canvas_for_init = canvas.clone();
    #[cfg(feature = "debug")]
    let window_for_debug = window.clone();

    Ok(wasm_bindgen_futures::future_to_promise(async move {
        let grid_handle =
            match JsFuture::from(jet_grid_wasm::init_renderer(canvas_for_init, dpr)).await {
                Ok(handle) => handle,
                Err(e) => {
                    record_error(&status, &e);
                    return Err(e);
                }
            };
        set_status_str(&status, "phase", "renderer-ready");
        let last_ops: Rc<RefCell<Option<Vec<PaintOp>>>> = Rc::new(RefCell::new(None));
        let highlight_index: Rc<RefCell<Option<usize>>> = Rc::new(RefCell::new(None));
        let selection: Rc<RefCell<CellSelectionState>> =
            Rc::new(RefCell::new(CellSelectionState::default()));
        let scroll_offsets: Rc<RefCell<ScrollOffsets>> =
            Rc::new(RefCell::new(ScrollOffsets::default()));
        let layout_tree: Rc<RefCell<LayoutTree>> = Rc::new(RefCell::new(repaint(
            &grid_handle,
            &handle,
            &renderer,
            &last_ops,
            &highlight_index,
            &selection,
            &scroll_offsets,
            &status,
        )?));
        let repaint_ctx = RepaintCtx {
            grid_handle: grid_handle.clone(),
            handle: handle.clone(),
            renderer: renderer.clone(),
            layout_tree: layout_tree.clone(),
            last_ops: last_ops.clone(),
            highlight_index: highlight_index.clone(),
            selection: selection.clone(),
            scroll_offsets: scroll_offsets.clone(),
            status: status.clone(),
        };
        install_async_update_scheduler(&window, repaint_ctx.clone());

        #[cfg(feature = "debug")]
        {
            let repaint_for_trigger = repaint_ctx.clone();
            let repaint_trigger: crate::debug::RepaintTrigger = Rc::new(move || {
                repaint_for_trigger.repaint_now("debug");
            });

            let bridge = DebugBridgeState {
                layout_tree: layout_tree.clone(),
                last_ops: last_ops.clone(),
                highlight_index: highlight_index.clone(),
            };
            let debug_handle = JetDebug::new(handle.clone(), bridge, repaint_trigger);
            js_sys::Reflect::set(
                &window_for_debug,
                &JsValue::from_str("__jet_debug"),
                &JsValue::from(debug_handle),
            )?;
        }

        install_click_listener(&window, &canvas, repaint_ctx.clone())?;
        install_wheel_listener(&window, &canvas, repaint_ctx.clone())?;
        install_copy_listener(
            &window,
            layout_tree.clone(),
            selection.clone(),
            status.clone(),
        )?;

        Ok(JsValue::from(JetWebGpuApp {
            grid_handle,
            status,
        }))
    }))
}

#[derive(Clone)]
struct RepaintCtx {
    grid_handle: JsValue,
    handle: Rc<MountHandle>,
    renderer: Rc<RefCell<Renderer<WebGpuBackend>>>,
    layout_tree: Rc<RefCell<LayoutTree>>,
    last_ops: Rc<RefCell<Option<Vec<PaintOp>>>>,
    highlight_index: Rc<RefCell<Option<usize>>>,
    selection: Rc<RefCell<CellSelectionState>>,
    scroll_offsets: Rc<RefCell<ScrollOffsets>>,
    status: js_sys::Object,
}

/// @spec .aw/tech-design/projects/jet/semantic/jet-wasm-src-react.md#schema
impl RepaintCtx {
    fn repaint_now(&self, reason: &str) {
        set_status_str(&self.status, "lastRepaintReason", reason);
        match repaint(
            &self.grid_handle,
            &self.handle,
            &self.renderer,
            &self.last_ops,
            &self.highlight_index,
            &self.selection,
            &self.scroll_offsets,
            &self.status,
        ) {
            Ok(new_tree) => *self.layout_tree.borrow_mut() = new_tree,
            Err(e) => record_error(&self.status, &e),
        }
    }
}

fn schedule_repaint(
    window: &web_sys::Window,
    pending: &Rc<Cell<bool>>,
    repaint_ctx: RepaintCtx,
    reason: &'static str,
) {
    bump_status_num(&repaint_ctx.status, "repaintRequests");
    set_status_str(&repaint_ctx.status, "lastRepaintRequestReason", reason);
    if pending.replace(true) {
        bump_status_num(&repaint_ctx.status, "coalescedRepaintRequests");
        return;
    }
    bump_status_num(&repaint_ctx.status, "scheduledRepaintRequests");
    set_status_bool(&repaint_ctx.status, "scheduledRepaintPending", true);

    let pending_for_frame = pending.clone();
    let repaint_for_frame = repaint_ctx.clone();
    let frame_cb = Closure::once_into_js(move || {
        pending_for_frame.set(false);
        set_status_bool(&repaint_for_frame.status, "scheduledRepaintPending", false);
        repaint_for_frame.repaint_now(reason);
    });
    let callback = frame_cb.unchecked_ref::<js_sys::Function>();
    if window.request_animation_frame(callback).is_err() {
        pending.set(false);
        set_status_bool(&repaint_ctx.status, "scheduledRepaintPending", false);
        repaint_ctx.repaint_now(reason);
    }
}

fn repaint(
    grid_handle: &JsValue,
    handle: &MountHandle,
    renderer: &Rc<RefCell<Renderer<WebGpuBackend>>>,
    last_ops: &Rc<RefCell<Option<Vec<PaintOp>>>>,
    highlight_index: &Rc<RefCell<Option<usize>>>,
    selection: &Rc<RefCell<CellSelectionState>>,
    scroll_offsets: &Rc<RefCell<ScrollOffsets>>,
    status: &js_sys::Object,
) -> Result<LayoutTree, JsValue> {
    let repaint_start_ms = browser_now_ms();
    let _ = handle.flush();
    let (new_lt, mut ops, scroll_bounds) = {
        let r = renderer.borrow();
        let snapshot = handle.snapshot();
        let scroll_bounds = renderer::scroll_bounds_for_id(&snapshot, r.viewport, "table-viewport");
        let scroll_offsets = scroll_offsets.borrow();
        let new_lt = renderer::layout_with_scroll_offsets(&snapshot, r.viewport, &scroll_offsets);
        let ops = renderer::paint(&new_lt, &r.theme);
        (new_lt, ops, scroll_bounds)
    };
    let visible_cells = visible_table_cells(&new_lt);
    ops.extend(selection.borrow().highlight_ops(&visible_cells));
    if let Some(idx) = *highlight_index.borrow() {
        if let Some(node) = new_lt.nodes.get(idx) {
            ops.push(PaintOp::StrokeRect {
                rect: node.rect,
                color: crate::renderer::Color {
                    r: 0xff,
                    g: 0x33,
                    b: 0x33,
                    a: 0xff,
                },
                width: 2.0,
            });
        }
    }
    if let Some(bounds) = scroll_bounds {
        let offset = scroll_offsets.borrow().get("table-viewport");
        ops.extend(renderer::scrollbar_paint_ops(bounds, offset));
    }
    let paint_op_count = ops.len();

    let (cells, text_runs, cell_count, text_run_count, unsupported_count) = {
        let mut r = renderer.borrow_mut();
        let dpr = r.viewport.dpr;
        r.backend.set_dpr(dpr);
        r.backend.execute(&ops);
        let frame = r.backend.last_frame().clone();
        (
            frame.to_float32_array(),
            frame.to_js_text_runs(),
            frame.cells.len(),
            frame.text_runs.len(),
            frame.unsupported.len(),
        )
    };
    let bridge_mode = call_render_frame(grid_handle, &cells, &text_runs)?;
    let text_glyph_count = read_text_glyph_count(grid_handle);
    let text_atlas_status = read_text_atlas_status(grid_handle);
    let gpu_timing_status = read_gpu_timing_status(grid_handle);
    let repaint_cpu_ms = (browser_now_ms() - repaint_start_ms).max(0.0);
    record_frame(
        status,
        bridge_mode,
        paint_op_count,
        repaint_cpu_ms,
        cell_count,
        text_run_count,
        text_glyph_count,
        unsupported_count,
        &text_atlas_status,
        &gpu_timing_status,
    );
    record_selection(status, &selection.borrow(), &visible_cells);
    record_scroll(status, &scroll_offsets.borrow(), scroll_bounds);
    *last_ops.borrow_mut() = Some(ops);
    Ok(new_lt)
}

fn browser_now_ms() -> f64 {
    let global = js_sys::global();
    let Ok(performance) = js_sys::Reflect::get(&global, &JsValue::from_str("performance")) else {
        return 0.0;
    };
    let Ok(now_value) = js_sys::Reflect::get(&performance, &JsValue::from_str("now")) else {
        return 0.0;
    };
    let Ok(now_fn) = now_value.dyn_into::<js_sys::Function>() else {
        return 0.0;
    };
    now_fn
        .call0(&performance)
        .ok()
        .and_then(|value| value.as_f64())
        .filter(|value| value.is_finite())
        .unwrap_or(0.0)
}

fn install_async_update_scheduler(window: &web_sys::Window, repaint_ctx: RepaintCtx) {
    let pending = Rc::new(Cell::new(false));
    let window_for_schedule = window.clone();
    set_update_scheduler(Some(Rc::new(move || {
        if pending.replace(true) {
            return;
        }

        let pending_for_frame = pending.clone();
        let repaint_for_frame = repaint_ctx.clone();

        let frame_cb = Closure::once_into_js(move || {
            pending_for_frame.set(false);
            if !repaint_for_frame.handle.flush() {
                return;
            }
            repaint_for_frame.repaint_now("state-update");
        });
        let callback = frame_cb.unchecked_ref::<js_sys::Function>();
        if window_for_schedule
            .request_animation_frame(callback)
            .is_err()
        {
            pending.set(false);
        }
    })));
}

/// Read `RendererHandle.lastTextGlyphCount()` via JS reflection. Used
/// to mirror the wasm-side glyph count into `window.__jet_webgpu_status`
/// so the browser e2e (T8) can distinguish encode-fired-empty from
/// encode-fired-with-glyphs. Returns 0 if the method is missing.
/// Slice #2191.
fn read_text_glyph_count(grid_handle: &JsValue) -> u32 {
    read_u32_method(grid_handle, "lastTextGlyphCount")
}

struct TextAtlasStatus {
    mode: String,
    upload_count: u32,
    width: u32,
    height: u32,
    nonzero_alpha_count: u32,
    last_plan_cache_hit: bool,
    plan_cache_hits: u32,
    plan_cache_misses: u32,
}

struct GpuTimingStatus {
    enabled: bool,
    last_frame_gpu_ms: Option<f64>,
}

fn read_text_atlas_status(grid_handle: &JsValue) -> TextAtlasStatus {
    TextAtlasStatus {
        mode: read_string_method(grid_handle, "lastTextAtlasMode")
            .unwrap_or_else(|| "unknown".to_string()),
        upload_count: read_u32_method(grid_handle, "lastTextAtlasUploadCount"),
        width: read_u32_method(grid_handle, "lastTextAtlasWidth"),
        height: read_u32_method(grid_handle, "lastTextAtlasHeight"),
        nonzero_alpha_count: read_u32_method(grid_handle, "lastTextAtlasNonZeroAlphaCount"),
        last_plan_cache_hit: read_bool_method(grid_handle, "lastTextPlanCacheHit"),
        plan_cache_hits: read_u32_method(grid_handle, "textPlanCacheHits"),
        plan_cache_misses: read_u32_method(grid_handle, "textPlanCacheMisses"),
    }
}

fn read_gpu_timing_status(grid_handle: &JsValue) -> GpuTimingStatus {
    GpuTimingStatus {
        enabled: read_bool_method(grid_handle, "gpuTimingEnabled"),
        last_frame_gpu_ms: read_optional_f64_method(grid_handle, "lastFrameGpuMs")
            .filter(|ms| ms.is_finite() && *ms >= 0.0),
    }
}

fn read_u32_method(grid_handle: &JsValue, name: &str) -> u32 {
    let Ok(method_value) = js_sys::Reflect::get(grid_handle, &JsValue::from_str(name)) else {
        return 0;
    };
    let Ok(method) = method_value.dyn_into::<js_sys::Function>() else {
        return 0;
    };
    let Ok(result) = method.call0(grid_handle) else {
        return 0;
    };
    result.as_f64().map(|n| n as u32).unwrap_or(0)
}

fn read_bool_method(grid_handle: &JsValue, name: &str) -> bool {
    let Ok(method_value) = js_sys::Reflect::get(grid_handle, &JsValue::from_str(name)) else {
        return false;
    };
    let Ok(method) = method_value.dyn_into::<js_sys::Function>() else {
        return false;
    };
    method
        .call0(grid_handle)
        .ok()
        .and_then(|result| result.as_bool())
        .unwrap_or(false)
}

fn read_optional_f64_method(grid_handle: &JsValue, name: &str) -> Option<f64> {
    let method_value = js_sys::Reflect::get(grid_handle, &JsValue::from_str(name)).ok()?;
    let method = method_value.dyn_into::<js_sys::Function>().ok()?;
    method.call0(grid_handle).ok()?.as_f64()
}

fn read_string_method(grid_handle: &JsValue, name: &str) -> Option<String> {
    let method_value = js_sys::Reflect::get(grid_handle, &JsValue::from_str(name)).ok()?;
    let method = method_value.dyn_into::<js_sys::Function>().ok()?;
    method.call0(grid_handle).ok()?.as_string()
}

fn install_click_listener(
    window: &web_sys::Window,
    canvas: &web_sys::HtmlCanvasElement,
    repaint_ctx: RepaintCtx,
) -> Result<(), JsValue> {
    let dragging: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));
    let repaint_pending = Rc::new(Cell::new(false));

    let canvas_for_rect = canvas.clone();
    let window_for_down = window.clone();
    let repaint_for_down = repaint_ctx.clone();
    let repaint_pending_for_down = repaint_pending.clone();
    let dragging_for_down = dragging.clone();
    let down_cb = Closure::wrap(Box::new(move |e: web_sys::MouseEvent| {
        let rect = canvas_for_rect.get_bounding_client_rect();
        let point = Point {
            x: e.client_x() as f32 - rect.left() as f32,
            y: e.client_y() as f32 - rect.top() as f32,
        };
        let visible_cells = visible_table_cells(&repaint_for_down.layout_tree.borrow());
        let Some(cell) = hit_test_visible_cell(&visible_cells, point) else {
            return;
        };

        repaint_for_down
            .selection
            .borrow_mut()
            .select_cell(cell.coord, e.shift_key());
        *dragging_for_down.borrow_mut() = true;
        e.prevent_default();
        schedule_repaint(
            &window_for_down,
            &repaint_pending_for_down,
            repaint_for_down.clone(),
            "selection",
        );
    }) as Box<dyn FnMut(_)>);
    canvas.add_event_listener_with_callback("mousedown", down_cb.as_ref().unchecked_ref())?;
    down_cb.forget();

    let canvas_for_rect = canvas.clone();
    let window_for_move = window.clone();
    let repaint_for_move = repaint_ctx.clone();
    let repaint_pending_for_move = repaint_pending.clone();
    let dragging_for_move = dragging.clone();
    let move_cb = Closure::wrap(Box::new(move |e: web_sys::MouseEvent| {
        if !*dragging_for_move.borrow() || e.buttons() != 1 {
            return;
        }
        let rect = canvas_for_rect.get_bounding_client_rect();
        let point = Point {
            x: e.client_x() as f32 - rect.left() as f32,
            y: e.client_y() as f32 - rect.top() as f32,
        };
        let visible_cells = visible_table_cells(&repaint_for_move.layout_tree.borrow());
        let Some(cell) = hit_test_visible_cell(&visible_cells, point) else {
            return;
        };

        repaint_for_move
            .selection
            .borrow_mut()
            .select_cell(cell.coord, true);
        e.prevent_default();
        schedule_repaint(
            &window_for_move,
            &repaint_pending_for_move,
            repaint_for_move.clone(),
            "selection",
        );
    }) as Box<dyn FnMut(_)>);
    canvas.add_event_listener_with_callback("mousemove", move_cb.as_ref().unchecked_ref())?;
    move_cb.forget();

    let canvas_for_rect = canvas.clone();
    let window_for_up = window.clone();
    let repaint_for_up = repaint_ctx.clone();
    let repaint_pending_for_up = repaint_pending.clone();
    let dragging_for_up = dragging.clone();
    let up_cb = Closure::wrap(Box::new(move |e: web_sys::MouseEvent| {
        if !*dragging_for_up.borrow() {
            return;
        }
        *dragging_for_up.borrow_mut() = false;
        let rect = canvas_for_rect.get_bounding_client_rect();
        let point = Point {
            x: e.client_x() as f32 - rect.left() as f32,
            y: e.client_y() as f32 - rect.top() as f32,
        };
        let visible_cells = visible_table_cells(&repaint_for_up.layout_tree.borrow());
        if let Some(cell) = hit_test_visible_cell(&visible_cells, point) {
            repaint_for_up
                .selection
                .borrow_mut()
                .select_cell(cell.coord, true);
        }
        e.prevent_default();
        schedule_repaint(
            &window_for_up,
            &repaint_pending_for_up,
            repaint_for_up.clone(),
            "selection",
        );
    }) as Box<dyn FnMut(_)>);
    canvas.add_event_listener_with_callback("mouseup", up_cb.as_ref().unchecked_ref())?;
    up_cb.forget();

    let dragging_for_leave = dragging.clone();
    let leave_cb = Closure::wrap(Box::new(move |_e: web_sys::MouseEvent| {
        *dragging_for_leave.borrow_mut() = false;
    }) as Box<dyn FnMut(_)>);
    canvas.add_event_listener_with_callback("mouseleave", leave_cb.as_ref().unchecked_ref())?;
    leave_cb.forget();

    let canvas_for_rect = canvas.clone();
    let click_cb = Closure::wrap(Box::new(move |e: web_sys::MouseEvent| {
        let rect = canvas_for_rect.get_bounding_client_rect();
        let x = e.client_x() as f32 - rect.left() as f32;
        let y = e.client_y() as f32 - rect.top() as f32;

        let point = Point { x, y };
        let cb = repaint_ctx
            .layout_tree
            .borrow()
            .hit_test_on_click(point)
            .cloned();
        let Some(cb) = cb else { return };
        cb.call(());

        if repaint_ctx.handle.flush() {
            repaint_ctx.repaint_now("click");
        }
    }) as Box<dyn FnMut(_)>);
    canvas.add_event_listener_with_callback("click", click_cb.as_ref().unchecked_ref())?;
    click_cb.forget();
    Ok(())
}

fn install_wheel_listener(
    window: &web_sys::Window,
    canvas: &web_sys::HtmlCanvasElement,
    repaint_ctx: RepaintCtx,
) -> Result<(), JsValue> {
    let canvas_for_rect = canvas.clone();
    let window_for_wheel = window.clone();
    let repaint_pending = Rc::new(Cell::new(false));
    let wheel_cb = Closure::wrap(Box::new(move |e: web_sys::WheelEvent| {
        let rect = canvas_for_rect.get_bounding_client_rect();
        let point = Point {
            x: e.client_x() as f32 - rect.left() as f32,
            y: e.client_y() as f32 - rect.top() as f32,
        };
        let Some(target_id) = scroll_target_at(&repaint_ctx.layout_tree.borrow(), point) else {
            return;
        };

        let mut offsets = repaint_ctx.scroll_offsets.borrow_mut();
        let mut offset = offsets.get(&target_id);
        offset.x = (offset.x + e.delta_x() as f32).max(0.0);
        offset.y = (offset.y + e.delta_y() as f32).max(0.0);
        let snapshot = repaint_ctx.handle.snapshot();
        let viewport = repaint_ctx.renderer.borrow().viewport;
        if let Some(bounds) = renderer::scroll_bounds_for_id(&snapshot, viewport, &target_id) {
            offset = bounds.clamp(offset);
        }
        offsets.set(target_id, offset);
        drop(offsets);
        e.prevent_default();
        schedule_repaint(
            &window_for_wheel,
            &repaint_pending,
            repaint_ctx.clone(),
            "wheel",
        );
    }) as Box<dyn FnMut(_)>);
    canvas.add_event_listener_with_callback("wheel", wheel_cb.as_ref().unchecked_ref())?;
    wheel_cb.forget();
    Ok(())
}

fn scroll_target_at(tree: &LayoutTree, point: Point) -> Option<String> {
    tree.nodes.iter().rev().find_map(|node| {
        if !point_in_rect(node.rect, point) {
            return None;
        }
        match &node.kind {
            crate::renderer::LaidOutKind::Intrinsic { props, .. }
                if props.id.as_deref() == Some("table-viewport") =>
            {
                props.id.clone()
            }
            _ => None,
        }
    })
}

fn point_in_rect(rect: crate::renderer::Rect, point: Point) -> bool {
    point.x >= rect.x && point.x < rect.x + rect.w && point.y >= rect.y && point.y < rect.y + rect.h
}

fn install_copy_listener(
    window: &web_sys::Window,
    layout_tree: Rc<RefCell<LayoutTree>>,
    selection: Rc<RefCell<CellSelectionState>>,
    status: js_sys::Object,
) -> Result<(), JsValue> {
    let window_for_clipboard = window.clone();
    let key_cb = Closure::wrap(Box::new(move |e: web_sys::KeyboardEvent| {
        if !(e.meta_key() || e.ctrl_key()) || e.key().to_ascii_lowercase() != "c" {
            return;
        }
        let visible_cells = visible_table_cells(&layout_tree.borrow());
        let Some(tsv) = selection.borrow().selected_tsv(&visible_cells) else {
            return;
        };
        if tsv.is_empty() {
            return;
        }

        e.prevent_default();
        set_status_str(&status, "lastCopiedTsv", &tsv);
        set_status_str(&status, "clipboardWriteState", "pending");
        set_status_str(&status, "clipboardWriteError", "");
        bump_status_num(&status, "copyCount");
        match write_clipboard_text(&window_for_clipboard, &tsv) {
            Ok(promise) => {
                let status_for_promise = status.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match JsFuture::from(promise).await {
                        Ok(_) => {
                            set_status_str(&status_for_promise, "clipboardWriteState", "fulfilled");
                            set_status_str(&status_for_promise, "clipboardWriteError", "");
                            bump_status_num(&status_for_promise, "clipboardWriteSuccessCount");
                        }
                        Err(err) => {
                            record_clipboard_error(&status_for_promise, "rejected", &err);
                        }
                    }
                });
            }
            Err(err) => {
                record_clipboard_error(&status, "sync-error", &err);
            }
        }
    }) as Box<dyn FnMut(_)>);
    window.add_event_listener_with_callback("keydown", key_cb.as_ref().unchecked_ref())?;
    key_cb.forget();
    Ok(())
}

fn write_clipboard_text(window: &web_sys::Window, text: &str) -> Result<js_sys::Promise, JsValue> {
    let navigator = js_sys::Reflect::get(window, &JsValue::from_str("navigator"))?;
    let clipboard = js_sys::Reflect::get(&navigator, &JsValue::from_str("clipboard"))?;
    let write_text = js_sys::Reflect::get(&clipboard, &JsValue::from_str("writeText"))?
        .dyn_into::<js_sys::Function>()
        .map_err(|_| JsValue::from_str("navigator.clipboard.writeText is not a function"))?;
    write_text
        .call1(&clipboard, &JsValue::from_str(text))?
        .dyn_into::<js_sys::Promise>()
        .map_err(|_| JsValue::from_str("navigator.clipboard.writeText did not return a Promise"))
}

fn call_render_frame(
    grid_handle: &JsValue,
    cells: &js_sys::Float32Array,
    text_runs: &js_sys::Array,
) -> Result<&'static str, JsValue> {
    let method = js_sys::Reflect::get(grid_handle, &JsValue::from_str("renderFrameWithText"))?
        .dyn_into::<js_sys::Function>()
        .map_err(|_| JsValue::from_str("RendererHandle.renderFrameWithText is not a function"))?;
    method.call2(grid_handle, cells, text_runs)?;
    Ok("text")
}

fn call_method0(target: &JsValue, name: &str) -> Result<(), JsValue> {
    let method = js_sys::Reflect::get(target, &JsValue::from_str(name))?
        .dyn_into::<js_sys::Function>()
        .map_err(|_| JsValue::from_str("method is not a function"))?;
    method.call0(target)?;
    Ok(())
}

const STATUS_KEY: &str = "__jet_webgpu_status";

fn init_status(window: &web_sys::Window) -> js_sys::Object {
    let status = js_sys::Object::new();
    let _ = js_sys::Reflect::set(window, &JsValue::from_str(STATUS_KEY), status.as_ref());
    status
}

fn record_frame(
    status: &js_sys::Object,
    bridge_mode: &str,
    paint_op_count: usize,
    repaint_cpu_ms: f64,
    cell_count: usize,
    text_run_count: usize,
    text_glyph_count: u32,
    unsupported_count: usize,
    text_atlas_status: &TextAtlasStatus,
    gpu_timing_status: &GpuTimingStatus,
) {
    let frames = js_sys::Reflect::get(status, &JsValue::from_str("frames"))
        .ok()
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    set_status_str(status, "phase", "rendered");
    set_status_str(status, "bridgeMode", bridge_mode);
    set_status_num(status, "frames", frames + 1.0);
    set_status_num(status, "lastPaintOpCount", paint_op_count as f64);
    set_status_num(status, "lastRepaintCpuMs", repaint_cpu_ms);
    set_status_num(status, "lastCellCount", cell_count as f64);
    set_status_num(status, "lastTextRunCount", text_run_count as f64);
    set_status_num(status, "lastTextGlyphCount", text_glyph_count as f64);
    set_status_num(status, "lastUnsupportedCount", unsupported_count as f64);
    set_status_str(status, "textAtlasMode", &text_atlas_status.mode);
    set_status_num(
        status,
        "lastTextAtlasUploadCount",
        text_atlas_status.upload_count as f64,
    );
    set_status_num(status, "lastTextAtlasWidth", text_atlas_status.width as f64);
    set_status_num(
        status,
        "lastTextAtlasHeight",
        text_atlas_status.height as f64,
    );
    set_status_num(
        status,
        "lastTextAtlasNonZeroAlphaCount",
        text_atlas_status.nonzero_alpha_count as f64,
    );
    set_status_bool(
        status,
        "lastTextPlanCacheHit",
        text_atlas_status.last_plan_cache_hit,
    );
    set_status_num(
        status,
        "textPlanCacheHits",
        text_atlas_status.plan_cache_hits as f64,
    );
    set_status_num(
        status,
        "textPlanCacheMisses",
        text_atlas_status.plan_cache_misses as f64,
    );
    set_status_bool(status, "gpuTimingEnabled", gpu_timing_status.enabled);
    set_status_bool(
        status,
        "gpuTimingSampleReady",
        gpu_timing_status.last_frame_gpu_ms.is_some(),
    );
    if let Some(ms) = gpu_timing_status.last_frame_gpu_ms {
        set_status_num(status, "lastFrameGpuMs", ms);
    } else {
        set_status_value(status, "lastFrameGpuMs", &JsValue::NULL);
    }
}

fn record_selection(
    status: &js_sys::Object,
    selection: &CellSelectionState,
    visible_cells: &[renderer::selection::VisibleTableCell],
) {
    let Some(range) = selection.normalized_range() else {
        set_status_bool(status, "selectionActive", false);
        set_status_str(status, "selectionRange", "");
        set_status_num(status, "lastSelectionCellCount", 0.0);
        return;
    };
    let visible_count = selection.selected_visible_cells(visible_cells).len();
    set_status_bool(status, "selectionActive", true);
    set_status_str(
        status,
        "selectionRange",
        &format!(
            "{},{}:{},{}",
            range.row_start, range.col_start, range.row_end, range.col_end
        ),
    );
    set_status_num(status, "lastSelectionCellCount", visible_count as f64);
}

fn record_scroll(
    status: &js_sys::Object,
    scroll_offsets: &ScrollOffsets,
    scroll_bounds: Option<renderer::ScrollBounds>,
) {
    let offset = scroll_offsets.get("table-viewport");
    set_status_num(status, "scrollLeft", offset.x as f64);
    set_status_num(status, "scrollTop", offset.y as f64);
    if let Some(bounds) = scroll_bounds {
        set_status_num(status, "scrollMaxLeft", bounds.max_x as f64);
        set_status_num(status, "scrollMaxTop", bounds.max_y as f64);
        set_status_bool(
            status,
            "scrollbarVisible",
            bounds.has_horizontal_scrollbar() || bounds.has_vertical_scrollbar(),
        );
    } else {
        set_status_num(status, "scrollMaxLeft", 0.0);
        set_status_num(status, "scrollMaxTop", 0.0);
        set_status_bool(status, "scrollbarVisible", false);
    }
}

fn record_error(status: &js_sys::Object, error: &JsValue) {
    set_status_str(status, "phase", "error");
    set_status_value(status, "error", error);
    web_sys::console::error_1(error);
}

fn record_clipboard_error(status: &js_sys::Object, state: &str, error: &JsValue) {
    set_status_str(status, "clipboardWriteState", state);
    set_status_str(status, "clipboardWriteError", &js_error_message(error));
    bump_status_num(status, "clipboardWriteErrorCount");
    web_sys::console::error_1(error);
}

fn js_error_message(error: &JsValue) -> String {
    if let Some(message) = error.as_string() {
        return message;
    }
    js_sys::Reflect::get(error, &JsValue::from_str("message"))
        .ok()
        .and_then(|message| message.as_string())
        .unwrap_or_else(|| "unknown clipboard error".to_string())
}

fn set_status_str(status: &js_sys::Object, key: &str, value: &str) {
    set_status_value(status, key, &JsValue::from_str(value));
}

fn set_status_num(status: &js_sys::Object, key: &str, value: f64) {
    set_status_value(status, key, &JsValue::from_f64(value));
}

fn set_status_bool(status: &js_sys::Object, key: &str, value: bool) {
    set_status_value(status, key, &JsValue::from_bool(value));
}

fn bump_status_num(status: &js_sys::Object, key: &str) {
    let value = js_sys::Reflect::get(status, &JsValue::from_str(key))
        .ok()
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    set_status_num(status, key, value + 1.0);
}

fn set_status_value(status: &js_sys::Object, key: &str, value: &JsValue) {
    let _ = js_sys::Reflect::set(status, &JsValue::from_str(key), value);
}
// CODEGEN-END
