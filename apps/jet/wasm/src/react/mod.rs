// SPEC-MANAGED: .aw/tech-design/projects/jet/semantic/jet-wasm-src-react.md#schema
// CODEGEN-BEGIN
//! Jet WASM host adapters for the shared component runtime.
//!
//! The hooks/fiber/mount/flush runtime lives in `ui-runtime`; Jet keeps
//! only the WASM host adapters here:
//!
//! - `webgpu_app`: the primary path, `Element -> LayoutTree -> PaintOp ->
//!   jet-grid-wasm`.
//! - `dom_app`: compatibility path for browser-native controls while renderer
//!   parity catches up.
//!
//! The exported React-like API is a renderer-neutral authoring/runtime model
//! reused by Jet WASM and future native desktop apps; concrete host adapters
//! decide how the element tree is painted.

pub use ui_runtime::*;

#[cfg(all(feature = "webgpu-app", target_arch = "wasm32"))]
pub mod webgpu_app;

#[cfg(all(feature = "dom-app", target_arch = "wasm32"))]
pub mod dom_app;
// CODEGEN-END
