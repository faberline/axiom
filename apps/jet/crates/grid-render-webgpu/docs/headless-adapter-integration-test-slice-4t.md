# Headless wgpu adapter integration test — Slice 4t

> Issue: #1738 · Parent epic: #1254 · Slice: 4t

## Problem

CI can't rely on a real GPU. The unit tests in this crate cover the
pure data-plane bits (struct layout, growth policy, path selection,
adapter-feature mask) but the cell-rect render path — shader compile,
viewport bind group, vertex draw, color write — has only been
exercised against live GPUs locally, gated behind `#[ignore]`.

wgpu exposes a software-fallback adapter (Vulkan llvmpipe on Linux
CI hosts; Metal/Vulkan on developer machines). Routing the cell-rect
pipeline through it and reading back a single pixel from the rendered
target is a tight smoke test for "did anything break the GPU path?"
that runs in CI without a real GPU and skips gracefully on hosts
where no adapter at all is reachable.

`WebGpuRenderer` itself is currently coupled to a `wgpu::Surface`
(constructor takes a window target, drives `Surface::configure`).
Wiring a surface-less variant would be a sizable refactor; the spirit
of the AC — "validate the cell-rect rendering path end-to-end against
a software adapter" — is met by a small public helper that owns a
device + offscreen render target + the cell-rect pipeline, and an
integration test that drives it.

## Scope

In:

- New `pub mod headless` exposing:
  - `pub fn request_smoke_adapter() -> Option<(wgpu::Instance, wgpu::Adapter)>`
    — tries `force_fallback_adapter: true` first (software path),
    falls back to the default adapter request. Returns `None` if
    neither succeeds (no adapter at all reachable — true headless
    host).
  - `pub struct HeadlessSmokeRenderer` — owns a `wgpu::Device`,
    `wgpu::Queue`, an `Rgba8UnormSrgb` offscreen target with
    `RENDER_ATTACHMENT | COPY_SRC` usage, the cell-rect pipeline
    rebuilt against that format (count=1, no MSAA — software
    adapters often refuse MSAA), and a `COPY_DST | MAP_READ`
    readback buffer sized to the target.
  - `pub async fn HeadlessSmokeRenderer::new(adapter, size_px) ->
    Result<Self, RendererError>`.
  - `pub async fn HeadlessSmokeRenderer::render_single_cell(
    &mut self, cell: CellInstance, clear: [f32; 4]) ->
    Result<Vec<u8>, RendererError>` — clears, draws one
    `CellInstance`, copies texture → buffer, maps the buffer,
    returns the RGBA8 bytes in row-major top-to-bottom order
    (row stride normalized — wgpu's mapped layout is padded to
    256-byte alignment per row; the helper strips the padding).
- New integration test `tests/headless_smoke.rs`:
  - Tries to acquire an adapter via `request_smoke_adapter`. If
    `None`, prints an informative skip message and returns (test
    passes — environment lacks an adapter, not a regression).
  - Builds an 8×8 `HeadlessSmokeRenderer`.
  - Draws a single `CellInstance` filling the entire surface with a
    distinctive color (e.g. RGBA `[1.0, 0.25, 0.5, 1.0]` — pink, so
    a stuck-at-white default would fail this).
  - Asserts the center pixel matches the requested color (within ε
    accounting for sRGB encoding).

Out:

- Refactoring `WebGpuRenderer` to be surface-optional. That's a
  bigger ergonomics change and not required to satisfy the AC's
  intent (validate the cell-rect rendering path on a software
  adapter in CI). A future slice can unify the two paths.
- MSAA in the headless path. Software adapters expose patchy MSAA
  support; the smoke test forces `count=1`. Slice 4p's MSAA toggle
  is exercised at unit-test level via the pipeline-cache test.
- Multi-cell / scroll / DPR coverage. Future slices can extend the
  same harness; this slice closes the "any rendering at all in CI"
  gap.

## Interface

```rust
pub mod headless {
    /// Try to acquire a wgpu adapter suitable for CI smoke testing.
    /// Returns the (instance, adapter) pair so the caller can build
    /// a `HeadlessSmokeRenderer` against it.
    pub fn request_smoke_adapter()
        -> Option<(wgpu::Instance, wgpu::Adapter)>;

    pub struct HeadlessSmokeRenderer { /* ... */ }

    impl HeadlessSmokeRenderer {
        pub async fn new(
            adapter: wgpu::Adapter,
            size_px: (u32, u32),
        ) -> Result<Self, RendererError>;

        pub async fn render_single_cell(
            &mut self,
            cell: CellInstance,
            clear: [f32; 4],
        ) -> Result<Vec<u8>, RendererError>;

        pub fn size_px(&self) -> (u32, u32);
    }
}
```

Skip semantics in the integration test: a missing adapter is a
*test pass* with a printed message ("no wgpu adapter available;
skipping headless smoke"). Anything else (adapter present but
device creation fails, draw error, readback mismatch) is a *fail*.

## Acceptance Criteria

- [x] `tests/headless_smoke.rs` constructs the cell-rect render
      pipeline against a software-preferred adapter via
      `HeadlessSmokeRenderer`.
- [x] Submits one frame with a single `CellInstance`.
- [x] Reads back the resulting texture and asserts the cell's fill
      color appears at the expected pixel (within sRGB ε).
- [x] Skips with informative message if no software adapter
      available.
- [x] `cargo test -p jet-grid-render-webgpu` passes (the
      integration test is in the default test set; it skips not
      fails on adapter-less hosts).
- [x] Module-level docs explain WHY a separate headless harness
      exists (CI can't run `WebGpuRenderer::new` — surface
      coupling — and the test must skip cleanly on truly headless
      hosts).

## Reference Context

- `apps/jet/crates/grid-render-webgpu/src/cell_rect.rs` — the shader
  source + `CellInstance` layout this harness rebuilds against an
  off-screen target.
- `apps/jet/crates/grid-render-webgpu/src/viewport.rs` —
  `ViewportUniforms` (Slice 4c, #1721); the smoke renderer creates
  its own uniform buffer + bind group.
- `apps/jet/crates/grid-render-webgpu/src/lib.rs:1648-1663` — the
  stubbed `build_headless_renderer` test helper (returns
  `RendererError::NoSurface`). This slice does NOT touch that path;
  the new `headless` module is a parallel surface-less pipeline.
- Parent epic #1254 — WebGPU-React renderer; this slice adds the
  CI-runnable smoke for the cell-rect path.
