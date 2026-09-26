//! Backend-mask selection for the wgpu `InstanceDescriptor`.
//!
//! Why this module exists: `wgpu::Backends::all()` is too wide a net
//! for either of our two targets:
//!
//! - **Web** (wasm32): includes WebGL2 (`GL`) as a fallback. On a
//!   browser without WebGPU support, the adapter request would
//!   *succeed* via WebGL2 — masking the missing capability behind a
//!   backend that has no compute, no timestamp queries, and totally
//!   different buffer ergonomics. We'd rather fail loudly so the
//!   React layer can show a "WebGPU not supported" UI than render
//!   an inconsistent best-effort frame.
//! - **Native**: includes `GL` on every platform with an OpenGL
//!   driver. GL on macOS is the legacy 4.1 stack; GL on Linux is
//!   the mesa compatibility layer. We want the modern trio
//!   (Metal/Vulkan/DX12) so what we ship matches what we test
//!   against.
//!
//! Invariant — target-conditional backend mask: every code path
//! that constructs a `wgpu::Instance` MUST pass
//! [`backends_for_target`] (or an equivalent constant). The mask is
//! a `cfg(target_arch = "wasm32")` branch, NOT a Cargo feature: we
//! don't want a host integration to be able to flip the web build
//! over to GLES by enabling a feature.
//!
//! Invariant — error messages name the backend: a developer who
//! hits "no compatible GPU adapter" needs to know which backend
//! space we searched before they go reinstall drivers. The
//! [`backend_description`] helper feeds [`crate::RendererError::NoAdapter`]'s
//! `Display`.

/// Backend mask the renderer should pass to `wgpu::Instance::new`
/// for the current build target.
///
/// - wasm32: `Backends::BROWSER_WEBGPU` (no WebGL2 fallback).
/// - everything else: `Backends::PRIMARY` (Metal | Vulkan | DX12 |
///   BROWSER_WEBGPU; the last entry is inert off-web).
///
/// Pure — testable without a GPU.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/backend-selection-slice-4l.md#interface
/// @issue #1730
pub fn backends_for_target() -> wgpu::Backends {
    #[cfg(target_arch = "wasm32")]
    {
        wgpu::Backends::BROWSER_WEBGPU
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        wgpu::Backends::PRIMARY
    }
}

/// Short human-readable name of the backend mask returned by
/// [`backends_for_target`]. Surfaced inside the
/// [`crate::RendererError::NoAdapter`] `Display` impl so a developer
/// hitting "no adapter" learns whether the renderer searched the
/// web's WebGPU surface or the native PRIMARY trio.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/backend-selection-slice-4l.md#interface
/// @issue #1730
pub fn backend_description() -> &'static str {
    #[cfg(target_arch = "wasm32")]
    {
        "WebGPU (BROWSER_WEBGPU)"
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        "Metal/Vulkan/DX12 (PRIMARY)"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_target_returns_primary_mask() {
        // The host CI runs native (not wasm32), so we exercise the
        // native branch here. The wasm32 branch is exercised by
        // wasm-bindgen-test in the jet-grid-wasm crate; here we
        // just lock the native invariant.
        #[cfg(not(target_arch = "wasm32"))]
        {
            assert_eq!(backends_for_target(), wgpu::Backends::PRIMARY);
            // PRIMARY explicitly excludes the legacy GL backend —
            // assert that so a future wgpu version that folds GL
            // into PRIMARY surfaces a test failure here.
            assert!(
                !backends_for_target().contains(wgpu::Backends::GL),
                "native build's backend mask must not include GL"
            );
        }
    }

    #[test]
    fn description_is_nonempty_and_target_specific() {
        let d = backend_description();
        assert!(!d.is_empty(), "backend_description must be non-empty");
        #[cfg(not(target_arch = "wasm32"))]
        {
            // Native description must namedrop the modern trio so a
            // developer hitting "no adapter" knows what was tried.
            assert!(
                d.contains("Metal"),
                "native description must mention Metal; got: {d}"
            );
            assert!(
                d.contains("Vulkan"),
                "native description must mention Vulkan; got: {d}"
            );
            assert!(
                d.contains("DX12"),
                "native description must mention DX12; got: {d}"
            );
        }
    }

    #[test]
    fn no_adapter_display_names_backend() {
        // Round-trip: the renderer's `NoAdapter` error must include
        // the description so devs aren't left guessing whether the
        // adapter search was WebGPU-only or PRIMARY-wide.
        let err = crate::RendererError::NoAdapter;
        let s = err.to_string();
        assert!(
            s.contains(backend_description()),
            "NoAdapter Display must include `{}`; got: {s}",
            backend_description()
        );
    }
}
