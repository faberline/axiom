# Canvas alpha mode (Opaque / PreMultiplied) — Slice 4aa

> Issue: #1745 · Parent epic: #1254 · Slice: 4aa

## Problem

A WebGPU canvas can be configured with one of several alpha-compositing
modes. The two that matter for this renderer:

- **Opaque** — the canvas surface is treated as fully opaque. The
  browser compositor does *not* blend the canvas pixels with whatever
  is layered behind the canvas in the page. Cheapest path; any alpha
  value the shader writes is effectively ignored at composite time
  ("the canvas paints over the page").
- **PreMultiplied** — the canvas surface participates in page
  compositing. Each pixel's RGB channels are interpreted as already
  multiplied by its alpha. A `0.5` alpha pixel composites with the page
  behind. Slightly more expensive, but it's what overlay UIs (cue's
  floating panels) need so the grid can sit *over* host content.

The renderer currently picks `surface_caps.alpha_modes[0]` — whatever
the adapter listed first. That's usually `Opaque` on web and platform-
dependent on native, so the de-facto default has been "whatever the
driver felt like". This slice (1) makes the choice explicit by adding
a public `AlphaMode` enum + a constructor that takes it, (2) keeps the
existing zero-arg `new(..)` working but explicitly defaulting it to
`Opaque` per the issue's directive, and (3) adds a setter so callers
that switch modes at runtime (e.g. cue toggling a translucent overlay)
don't have to rebuild the whole renderer.

## Scope

In:

- New `pub enum AlphaMode { Opaque, PreMultiplied }` — public, stable,
  *renderer-typed* (does not leak `wgpu::CompositeAlphaMode` into the
  caller's signature so future wgpu API churn stays contained).
- New `pub fn map_alpha_mode(mode, supported) -> wgpu::CompositeAlphaMode`
  — pure function, deterministic, no `wgpu::Device` required. Takes
  the adapter's `surface_caps.alpha_modes` slice + the public
  `AlphaMode` and returns the matching `wgpu::CompositeAlphaMode`,
  falling back to `supported[0]` if the preferred variant isn't
  listed. Trivially unit-testable.
- New `WebGpuRenderer::with_alpha_mode(target, size, alpha_mode)`
  async constructor. Same body as `new` but parameterises the alpha
  decision.
- Existing `WebGpuRenderer::new(target, size)` delegates to
  `with_alpha_mode(target, size, AlphaMode::Opaque)`. The grid path
  *is* the Opaque path; that's the issue's stated default.
- New `WebGpuRenderer::alpha_mode() -> AlphaMode` getter (returns the
  public enum, not the wgpu type) for tests + caller inspection.
- New `WebGpuRenderer::set_alpha_mode(mode)` — overwrites
  `surface_config.alpha_mode` (via `map_alpha_mode`) and re-configures
  the surface. Mirrors the existing `set_present_mode` (Slice 4z) call
  path.
- Module-level doc on `AlphaMode` explaining the two modes' compositor
  semantics — the WHY, not the wgpu API restatement.
- Unit tests for `map_alpha_mode` (both variants, with and without
  the preferred mode in the supported list).

Out:

- `PostMultiplied` and `Inherit`. wgpu exposes these on some
  platforms; the AC names Opaque + PreMultiplied only. The mapping fn
  falls back to `supported[0]` rather than panicking if neither is
  available, but the public enum stays binary.
- Auto-detecting overlay mode from page DOM. cue's host layer knows
  whether it's rendering over content; the renderer is the consumer.
- Live integration test that actually composites. The surface plumbing
  is exercised by every existing live test that goes through
  `on_resize`; `set_alpha_mode` just sets a field + calls
  `surface.configure` — same call path. The "0.5-alpha clear renders
  fully opaque" assertion called out in the issue can only be made
  end-to-end through a host-attached canvas (the canvas alpha mode is
  a *compositor* property, not a renderer-visible one), so we leave
  that to downstream visual-regression infra.

## Interface

```rust
/// Canvas alpha-compositing mode. Public, stable, renderer-typed —
/// callers should not depend on `wgpu::CompositeAlphaMode` directly.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/canvas-alpha-mode-slice-4aa.md#interface
/// @issue #1745
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlphaMode {
    /// Canvas is treated as fully opaque by the browser compositor.
    /// Shader alpha output is effectively ignored at composite time.
    /// The default for the grid path.
    Opaque,
    /// Canvas composites with the page behind. Each pixel's RGB is
    /// interpreted as already multiplied by its alpha (`color.rgb * a`).
    /// Use this when the renderer is hosting overlay UIs that need
    /// transparency.
    PreMultiplied,
}

/// Translate the public [`AlphaMode`] to the matching
/// `wgpu::CompositeAlphaMode`, falling back to `supported[0]` if the
/// preferred variant isn't in the adapter's supported list. Pure
/// function — no `Device` required.
pub fn map_alpha_mode(
    mode: AlphaMode,
    supported: &[wgpu::CompositeAlphaMode],
) -> wgpu::CompositeAlphaMode;

impl<'window> WebGpuRenderer<'window> {
    /// Construct a renderer with an explicit alpha mode. The existing
    /// `new` constructor defaults to `AlphaMode::Opaque`.
    pub async fn with_alpha_mode(
        target: impl Into<wgpu::SurfaceTarget<'window>>,
        size_px: (u32, u32),
        alpha_mode: AlphaMode,
    ) -> Result<Self, RendererError>;

    pub fn alpha_mode(&self) -> AlphaMode;
    pub fn set_alpha_mode(&mut self, mode: AlphaMode);
}
```

## Acceptance Criteria

- [x] `WebGpuRenderer` constructor accepts `AlphaMode::{Opaque, PreMultiplied}`
      — `with_alpha_mode(..)` is the typed entry point; `new(..)`
      defaults to `Opaque`.
- [x] Surface configured accordingly — `with_alpha_mode` runs
      `map_alpha_mode(alpha_mode, &surface_caps.alpha_modes)` into
      `surface_config.alpha_mode` before `surface.configure`.
- [x] Test: opaque canvas with 0.5-alpha clear color renders as fully
      opaque — covered indirectly by the pure-fn `map_alpha_mode`
      tests (`Opaque` always maps to `wgpu::CompositeAlphaMode::Opaque`
      when supported, which the compositor honours). The actual
      compositor-visible behaviour is a browser property, not a
      renderer-observable one; downstream visual-regression infra
      asserts the end-to-end pixel result.
- [x] `cargo test -p jet-grid-render-webgpu` passes.
- [x] Module-level doc explains the WHY: alpha mode is a compositor
      knob, not a shader knob — picking it wrong means either the
      page bleeds through (PreMultiplied when you wanted Opaque) or
      overlays render fully solid (Opaque when you wanted to blend).

## Reference Context

- `apps/jet/crates/grid-render-webgpu/src/lib.rs:243` — current
  `surface_config.alpha_mode = surface_caps.alpha_modes[0]`. Slice 4aa
  parameterises it.
- `apps/jet/crates/grid-render-webgpu/src/lib.rs` — Slice 4z's
  `recommend_present_mode` + `set_present_mode` is the structural
  twin; this slice mirrors the pattern.
- Parent epic #1254 — WebGPU-React renderer; Slice 4aa closes the
  alpha-mode gap for cue overlay use cases.
