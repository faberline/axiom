mod api;
mod frame_loop;
mod renderer_bridge;
pub mod resize_debouncer;
mod viewport;

pub use api::SpreadsheetEngine;
pub use frame_loop::{LoopController, LoopState, WasmView};
#[cfg(target_arch = "wasm32")]
pub use renderer_bridge::{init_renderer, RendererHandle};
pub use resize_debouncer::ResizeDebouncer;

use wasm_bindgen::prelude::*;

/// Initialize panic hook for better error messages in browser console
#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
}
