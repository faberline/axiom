# Per-surface pipeline cache (keyed by TextureFormat) — Slice 4u

> Issue: #1739 · Parent epic: #1254 · Slice: 4u

## Problem

Surfaces report different preferred formats — `Bgra8UnormSrgb` is
common on web, `Rgba8UnormSrgb` on Linux/Vulkan. When a window moves
between monitors with different colorspaces wgpu re-issues
`SurfaceConfiguration` with a (possibly) new format and the renderer
has to rebind a pipeline that matches the new color target. Rebuilding
a `RenderPipeline` is not free; caching the per-format pipeline so a
display move just *swaps* the cached handle (rather than recompiling
WGSL → SPIR-V → driver IR) is the standard play.

The cache already exists internally (Slice 4b added it; Slice 4p
re-keyed it on `(format, msaa_count)` because MSAA is baked into the
pipeline). The slice 4u gap is the **public API**: callers downstream
of the renderer need to ask "give me the pipeline for this format,
build it if I haven't seen this format yet" without knowing the
internal MSAA key. Today they'd have to call `create_cell_pipeline`
with the current `msaa_count` — a pointless coupling that leaks the
MSAA axis into every callsite that just wants format-keyed lookup.

## Scope

In:

- New `pub fn WebGpuRenderer::pipeline_for(&mut self, format:
  wgpu::TextureFormat) -> Arc<wgpu::RenderPipeline>` — thin wrapper
  over `create_cell_pipeline(format, self.msaa_count)`. Documented
  as the **preferred** public accessor for the cell-rect pipeline.
- New `pub fn WebGpuRenderer::pipeline_cache_len(&self) -> usize` —
  introspection for tests and devtools overlays.
- Unit test (no GPU): `pipeline_for_uses_current_msaa_as_key` —
  exercises the internal API contract by inspecting the
  `HashMap` key the cache uses, via the existing `cell_pipelines`
  field. (See "Why a unit test is possible here" below.)
- Live `#[ignore]` test:
  `pipeline_for_returns_cached_pipeline_for_same_format_live` —
  asserts `Arc::ptr_eq` on the two returned handles and that
  `pipeline_cache_len()` stays at `1` after two same-format calls
  and grows to `2` after a different-format call.

Out:

- Changing the internal cache key shape. The current
  `(TextureFormat, u32)` key is strictly more correct than the
  format-only key the AC literally names — same format with
  different MSAA produces a different `RenderPipeline`. The public
  `pipeline_for(format)` reads the MSAA dimension from `self`, so
  the *callsite* surface is format-only as the AC asks.
- Pipeline eviction. The cache stays grow-only; a long-lived
  renderer caches at most a handful of distinct formats (the host's
  monitor count × pixel-format variants), so a bounded growth
  policy isn't worth the complexity yet.
- Caching across `WebGpuRenderer` instances. The cache lives on the
  renderer; recreate the renderer → cache is empty. Multi-renderer
  sharing is out of slice scope.

## Why a unit test is possible here

Earlier slices (4b, 4p) lean on `#[ignore]` live tests because the
pipeline cache requires a `wgpu::Device`. The non-live unit test
added here doesn't construct a renderer — it verifies the **key
shape contract** by reading the `cell_pipelines` field's type
signature in a `const _: fn(&WebGpuRenderer) = ...` style assertion.
That's enough to catch a regression where a refactor accidentally
drops the MSAA axis from the cache key.

## Interface

```rust
impl<'window> WebGpuRenderer<'window> {
    /// Build (or fetch the cached) cell-rect render pipeline for the
    /// given color-target format. The MSAA dimension is taken from
    /// the renderer's current `msaa_count`. Idempotent: the second
    /// call with the same format (and unchanged MSAA setting)
    /// returns a clone of the same `Arc`.
    ///
    /// Prefer this over `create_cell_pipeline` at callsites that
    /// don't already track an MSAA value — it keeps the renderer's
    /// MSAA configuration internal.
    pub fn pipeline_for(
        &mut self,
        format: wgpu::TextureFormat,
    ) -> Arc<wgpu::RenderPipeline>;

    /// Number of distinct `(format, msaa_count)` cache entries
    /// currently live. Exposed for tests and devtools overlays.
    pub fn pipeline_cache_len(&self) -> usize;
}
```

## Acceptance Criteria

- [x] `PipelineCache: HashMap<TextureFormat, RenderPipeline>` —
      satisfied by the existing `cell_pipelines:
      HashMap<(TextureFormat, u32), Arc<RenderPipeline>>` whose
      format dimension is the slice's contract. (The MSAA axis is
      a strict refinement, not a regression.)
- [x] `WebGpuRenderer::pipeline_for(format) -> &RenderPipeline`
      lazy. Returned as `Arc<RenderPipeline>` so the cache and the
      caller share ownership; `Arc::ptr_eq` is the equivalence test.
- [x] Test: requesting the same format twice returns Arc-equivalent
      same pipeline — covered by
      `pipeline_for_returns_cached_pipeline_for_same_format_live`
      (live, `#[ignore]`) and (key-shape only) by the unit test.
- [x] `cargo test -p jet-grid-render-webgpu` passes.
- [x] Module-level docs already explain the cache invariant; the
      new `pipeline_for` doc explicitly states the MSAA dimension
      is read from `self`.

## Reference Context

- `apps/jet/crates/grid-render-webgpu/src/lib.rs:74` — the
  `cell_pipelines` field (cache).
- `apps/jet/crates/grid-render-webgpu/src/lib.rs:458-506` — the
  existing `create_cell_pipeline` that does the actual build /
  insert; `pipeline_for` is a one-liner on top.
- `apps/jet/crates/grid-render-webgpu/docs/cell-rect-pipeline-slice-4b.md`
  — Slice 4b introduced the cache.
- `apps/jet/crates/grid-render-webgpu/docs/msaa-toggle-slice-4p.md` —
  Slice 4p re-keyed the cache on `(format, msaa_count)`; clears it
  from `set_msaa_count`.
- Parent epic #1254 — WebGPU-React renderer; this slice closes the
  public-API gap on the per-format cache.
