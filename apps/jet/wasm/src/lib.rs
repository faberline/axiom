// SPEC-MANAGED: .aw/tech-design/projects/jet/semantic/jet-wasm-src.md#schema
// CODEGEN-BEGIN
//! jet-wasm — framework-agnostic browser-runtime substrate.
//!
//! Two layers:
//!
//! - **Generic substrate** (`surface` + `renderer`):
//!   - `Element` tree, `Props`, `Callback`, `Component`, and semantic
//!     `SurfaceSnapshot` types that any framework-specific runtime uses.
//!   - `renderer` module: layout + paint operations consumed by the
//!     browser WebGPU app renderer. A test-only paint backend remains
//!     available for host-side renderer unit tests.
//!   - Nothing here is React-specific. Vue / Angular / Solid /
//!     vanilla-JS adapters would target the same `Element` shape.
//!
//! - **Framework-specific runtimes** (feature-gated modules):
//!   - `react` (default feature): fiber + hooks + mount/flush
//!     commit loop. Target of jet's TSX → Rust transpiler.
//!   - `vue` (future): reactivity + templates.
//!   - `angular` (future): signals + zones.
//!   - `solid` (future): fine-grained reactivity.
//!
//! The framework-specific modules are **adapters over the same
//! generic substrate** — they all produce `surface` `Element` trees the
//! renderer consumes. Adding a new framework means adding a new
//! module here + a new compiler front-end in `jet::*_to_rust`.
//!
//! For the transpiler view of the world, a TSX function component
//! like:
//!
//! ```tsx
//! function Counter() {
//!   const [n, setN] = useState(0);
//!   return <button onClick={() => setN(n + 1)}>{n}</button>;
//! }
//! ```
//!
//! lowers to Rust that uses `jet_wasm::react::use_state` + builds
//! a `jet_wasm::Element` tree. Vue code would use
//! `jet_wasm::vue::ref` + build the same `Element` shape. Only
//! the reactive plumbing differs; the paint pipeline is shared.
//!
//! Deferred (scoped to later cycles):
//! - `use_effect` (needs async executor).
//! - Reconciliation diffing — current commit rebuilds the tree.
//! - Context, Suspense, refs, memo, error boundaries, concurrent mode.
//! - Taffy flexbox, rustybuzz text shaping, a11y shadow tree.
//! - Vue / Angular / Solid adapters.

pub mod renderer;

/// React-compat binding manifest — `jet.declare.d.ts` parsing + defaults.
///
/// @spec .aw/tech-design/projects/jet/wasm-renderer/binding-manifest.md
///
/// Public surface for the module's import-resolver consumers (the
/// transpiler will import these types when `jet-tsx-to-rust` lands as
/// a Phase 1 deliverable). This crate owns parsing + overlay-merge
/// only; transpiler emit semantics live in `transpiler.md`.
pub mod manifest;

#[cfg(feature = "host-bridge")]
pub mod host;

/// Text shaping engine — rustybuzz integration.
///
/// @spec .aw/tech-design/projects/jet/wasm-renderer/text-shaping.md
///
/// Phase 6a: glyph shaping + per-paragraph cache + paint-runtime
/// integration boundary. Line breaking, bidi, selection, IME, and
/// clipboard are deferred to Phase 6b–6f follow-up issues.
pub mod text;

#[cfg(feature = "react")]
pub mod react;

#[cfg(feature = "debug")]
pub mod debug;

pub use surface::{
    Callback, Component, ComponentFn, Element, Props, SurfaceNode, SurfaceNodeKind, SurfaceProps,
    SurfaceRect, SurfaceSnapshot,
};
// CODEGEN-END
