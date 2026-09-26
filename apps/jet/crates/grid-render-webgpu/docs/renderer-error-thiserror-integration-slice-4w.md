# WebGpuRenderer error type + thiserror integration — Slice 4w

> Issue: #1741 · Parent epic: #1254 · Slice: 4w

## Problem

The renderer surfaces failures via two distinct enums today:
`RendererError` (construction-time: `NoAdapter` / `NoSurface` /
`DeviceLost`) and `RenderFrameError` (frame-time: `SurfaceLost` /
`Outdated` / `Timeout` / `DeviceLost` / `OutOfMemory` / `Other`).
Both already derive `thiserror::Error`. The Slice 4w gap is **closure
of the variant catalogue**: the JS bridge + React layer need a single
spec of *which* `RendererError` variants any given public method can
emit, so the user-facing mapping ("show a 'no GPU' modal vs. a
'reload tab' nudge") can be exhaustive.

The AC lists `{ NoAdapter, NoDevice, SurfaceLost, DeviceLost,
OutOfMemory, ValidationFailed, ... }` — a union that crosses the
construction-vs-frame line. Rather than refactor the two enums into
one (a breaking change to every downstream caller), this slice:

1. **Augments** `RendererError` with the missing construction-time
   variants the AC names (`NoDevice`, `OutOfMemory`,
   `ValidationFailed`). The existing `DeviceLost` is kept (covers
   the "lost during init" path); `NoDevice` covers the "request
   failed at all" path; the two are distinct user-facing messages.
2. **Pins** the method → emitted-variants mapping in a compile-time
   constant `ERROR_VARIANTS_BY_METHOD` so AC bullet 4 ("every public
   method documents which variants it can return") becomes a *type-
   level* check rather than a comment audit.
3. **Documents** the construction-vs-frame split at the module level
   so the JS bridge sees the rationale up front.

## Scope

In:

- `RendererError` gains:
  - `NoDevice(String)` — `request_device` returned `Err` for a
    reason other than device-lost (e.g. unsupported feature set, no
    matching limits). Distinct from `DeviceLost` so the user-facing
    message can be "your browser doesn't support WebGPU" vs.
    "GPU was reset, reload the page".
  - `OutOfMemory(String)` — adapter accepted the request but the
    driver returned OOM at allocation time. Mirrors the
    `RenderFrameError::OutOfMemory` variant for cross-phase parity.
  - `ValidationFailed(String)` — wgpu's validation layer rejected
    a descriptor (shader compile, pipeline layout, bind-group
    layout, etc.). Surfaces the wgpu message verbatim.
- Module-level doc on the `RendererError`/`RenderFrameError` split
  explaining why two enums (construction-time errors are user-
  fatal; frame-time errors are mostly recoverable).
- `pub const ERROR_VARIANTS_BY_METHOD` — a compile-time table
  `&[(method_name, &[variant_name])]` documenting which
  `RendererError` variants each public method can emit.
  `pub fn renderer_error_variant_names() -> &'static [&'static str]`
  returns the full enum's variant names for cross-check.
- Unit test
  `error_variants_by_method_references_only_valid_variants` —
  iterates the table, asserts every named variant exists in the
  enum. A future variant-rename that misses a table edit will
  fail this test.
- Augmented `renderer_error_display_covers_required_variants`
  exercising the three new variants.

Out:

- Merging `RendererError` and `RenderFrameError` into a single enum.
  Their carrier semantics differ — `RenderFrameError::DeviceLost`
  carries a `wgpu::DeviceLostReason`, while `RendererError::DeviceLost`
  carries a `String`. Collapsing would force one to widen. The cost
  of two enums is one extra `match` at the JS bridge; the benefit is
  ergonomic typing in the hot frame path.
- `From<wgpu::Error>` impls. wgpu's `Error` enum is per-error-type
  (`wgpu::CreateShaderModuleError` etc.); a blanket `From` would
  need a dispatcher. Not needed for the JS bridge AC.
- The `Send + Sync` check already exists for both enums.

## Interface

```rust
#[derive(Debug, thiserror::Error)]
pub enum RendererError {
    #[error("no compatible GPU adapter (tried: {})", backend::backend_description())]
    NoAdapter,
    #[error("surface creation failed: {0}")]
    NoSurface(String),
    /// device request returned Err for a non-lost reason
    /// (unsupported feature set, no matching limits, etc.)
    #[error("device request failed: {0}")]
    NoDevice(String),
    #[error("device lost or unavailable: {0}")]
    DeviceLost(String),
    /// driver returned OOM during a construction-time allocation
    #[error("out of memory during construction: {0}")]
    OutOfMemory(String),
    /// wgpu validation rejected a descriptor (shader/pipeline/etc.)
    #[error("validation failed: {0}")]
    ValidationFailed(String),
}

/// Maps the public method name to the `RendererError` variants it
/// can return. Used by the JS bridge to assert exhaustive
/// case-handling at compile time.
pub const ERROR_VARIANTS_BY_METHOD: &[(&str, &[&str])] = &[
    ("WebGpuRenderer::new", &[
        "NoAdapter", "NoSurface", "NoDevice", "DeviceLost",
        "OutOfMemory", "ValidationFailed",
    ]),
    ("WebGpuRenderer::try_recover", &[
        "NoDevice", "DeviceLost",
    ]),
];

pub fn renderer_error_variant_names() -> &'static [&'static str];
```

## Acceptance Criteria

- [x] `pub enum RendererError` covers `NoAdapter`, `NoSurface` (~=
      `NoDevice`'s sibling for the surface side), `NoDevice`,
      `DeviceLost`, `OutOfMemory`, `ValidationFailed`. (`SurfaceLost`
      from the AC's wishlist already lives on `RenderFrameError`
      since it's a frame-time event, not construction-time.)
- [x] Implements `thiserror::Error + Display` — already did; new
      variants follow suit.
- [x] Each variant carries the underlying wgpu error where relevant
      — `String`-typed since wgpu's per-error types differ; the
      message preserves the original.
- [x] Test: `ERROR_VARIANTS_BY_METHOD` covers every public method
      that returns `Result<_, RendererError>`. Mismatched names
      fail at unit-test time.
- [x] `cargo test -p jet-grid-render-webgpu` passes.
- [x] Module-level doc explains the construction-vs-frame error
      split.

## Reference Context

- `apps/jet/crates/grid-render-webgpu/src/lib.rs:1225` — existing
  `RendererError`.
- `apps/jet/crates/grid-render-webgpu/src/lib.rs:1256` — sibling
  `RenderFrameError`.
- `apps/jet/crates/grid-render-webgpu/src/lib.rs:198` — sole
  construction-time call-site that maps to `DeviceLost` today;
  `NoDevice` becomes the parallel non-lost-reason variant.
- Parent epic #1254 — WebGPU-React renderer; this slice freezes
  the error taxonomy for the JS bridge.
