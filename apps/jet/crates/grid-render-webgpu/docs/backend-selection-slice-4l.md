# Backend selection — Slice 4l

> **Issue**: #1730
> **Parent epic**: #1254 (WebGPU-React renderer)
> **Depends on**: #1719 (Slice 4a — renderer wrapper).
> **Status**: in-flight

## Problem

wgpu's `Backends::all()` is a wide net — on web it ranges over
WebGPU + GLES; on native it spans Vulkan + Metal + DX12 + GL. The
renderer's promise is *crisp, predictable* GPU rendering, and the
fall-back backends weaken that promise in two specific ways:

1. **Web**: `Backends::all()` includes `GL` (WebGL2 fallback). On a
   browser without WebGPU support the adapter request *succeeds*
   via WebGL2, masking the missing capability behind a backend that
   doesn't speak compute, lacks timestamp queries, and has wildly
   different upload/buffer ergonomics. We'd rather fail loudly so
   the React layer can show a "WebGPU not supported" UI than render
   an inconsistent best-effort frame.
2. **Native**: `Backends::all()` includes `GL` on every platform
   that has an OpenGL driver (Linux/macOS). GL on macOS is the
   legacy 4.1 stack; on Linux it's mesa's compatibility layer. We
   want the modern PRIMARY trio (Metal/Vulkan/DX12) so behavior
   matches what shipping users see.

The fix is structural: pin `Backends` to a target-conditional
constant — `BROWSER_WEBGPU` for wasm32, `PRIMARY` for everything
else — and surface a clear error message naming the backend mask
when the adapter request fails so devs aren't left guessing whether
the failure was a driver issue or a backend-mask issue.

## Scope

In:

- New `backend` module exposing:
  - `backends_for_target() -> wgpu::Backends` — `cfg(target_arch =
    "wasm32")` branch returns `Backends::BROWSER_WEBGPU`; the
    fallback returns `Backends::PRIMARY`. Pure — testable without
    a GPU.
  - `backend_description() -> &'static str` — human-readable name
    of the backend mask the renderer is configured for ("WebGPU"
    on wasm32, "Metal/Vulkan/DX12 (PRIMARY)" on native). Used in
    error messages so devs know *which* backend space the adapter
    request searched.
- `WebGpuRenderer::new` switches from `Backends::all()` to
  `backend::backends_for_target()` on the `InstanceDescriptor`.
- The existing `RendererError::NoAdapter` variant gains the
  backend description in its `Display` output:
  `"no compatible GPU adapter (tried: <description>)"`. The variant
  signature itself stays unchanged so external matchers don't
  regress.

Out:

- Per-call backend override. A future slice can take a constructor
  argument if a host integration wants e.g. force-WebGL2 for
  testing; that's a separate AC.
- Runtime probing of `Backends::SECONDARY`. wgpu's GL backend on
  native is intentionally excluded for the reasons above; if a
  user has a Vulkan-incapable machine, "WebGPU unavailable" is
  the right signal.
- Build-time feature flags. The cfg branch is on `target_arch`,
  not a Cargo feature — every native build behaves the same; every
  wasm build behaves the same.

## Interface

```rust
// apps/jet/crates/grid-render-webgpu/src/backend.rs

/// Backend mask the renderer should pass to `wgpu::Instance::new`
/// for the current build target.
///
/// - wasm32: `Backends::BROWSER_WEBGPU` (no WebGL2 fallback).
/// - everything else: `Backends::PRIMARY` (Metal | Vulkan | DX12).
pub fn backends_for_target() -> wgpu::Backends;

/// Short human-readable name of the backend mask returned by
/// [`backends_for_target`]. Used in error messages so the dev who
/// hits "no adapter" knows whether the renderer searched WebGPU
/// only (web) or the PRIMARY trio (native).
pub fn backend_description() -> &'static str;
```

`WebGpuRenderer::new` and `RendererError::NoAdapter` are unchanged
in shape; only the Display string for `NoAdapter` gains the
`(tried: <description>)` suffix.

## Acceptance Criteria

- [x] Web build: `InstanceDescriptor` with `backends =
      BROWSER_WEBGPU` — `cfg(target_arch = "wasm32")` branch of
      `backends_for_target` returns the right constant; renderer
      consumes it.
- [x] Native build: `InstanceDescriptor` with `backends = PRIMARY`
      — fallback branch returns the right constant.
- [x] On adapter request failure: clear error explaining which
      backend was tried — `NoAdapter`'s `Display` now includes
      `(tried: <description>)` via `backend_description()`.
- [x] `cargo test -p jet-grid-render-webgpu` passes.
- [x] Module-level docs explain WHY (predictable fallback behaviour
      vs `Backends::all()` over-broad mask).

## Reference context

- `apps/jet/crates/grid-render-webgpu/src/lib.rs` — `WebGpuRenderer::new`
  (where the `InstanceDescriptor` is built), `RendererError`.
- wgpu 24 docs:
  - `wgpu::Backends::BROWSER_WEBGPU` — the Web build's WebGPU-only
    mask. Excludes WebGL2.
  - `wgpu::Backends::PRIMARY` — `VULKAN | METAL | DX12 |
    BROWSER_WEBGPU`. Native code paths use the first three;
    BROWSER_WEBGPU is inert off-web.
