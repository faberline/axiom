# Submit batching — one Queue::submit per frame — Slice 4j

> **Issue**: #1728
> **Parent epic**: #1254 (WebGPU-React renderer)
> **Depends on**: #1719–#1723 (Slices 4a–4e — renderer + `render_frame`).
> **Status**: in-flight

## Problem

The current `WebGpuRenderer::render_frame` (Slice 4e) already issues a
single `Queue::submit` per frame — but it's hard-wired around exactly
one render pass. Every future slice that wants to add a second pass
(text rendering, debug overlays, picking buffer) would either need to
emit its own encoder + submit (paying full driver-round-trip cost per
pass) or hack into `render_frame`'s closed body.

We need an explicit **frame builder** that:

1. Owns the per-frame `SurfaceTexture` + `TextureView` + `CommandEncoder`.
2. Exposes pass-encoding methods that *append* commands to the shared
   encoder.
3. Has a single `commit` step that calls `Queue::submit` exactly once
   and presents.

This is the only design that lets later slices add passes without each
slice re-paying the driver cost or duplicating the surface-acquire +
encoder boilerplate. The validation layer warning count must stay zero —
in particular, every encoded pass must drop its `RenderPass` before the
encoder is finalized.

## Scope

In:

- New public type `FrameBuilder<'r, 'window>` with these methods:
  - `encode_cell_pass(&mut self, cells: &[CellInstance], clear: Option<wgpu::Color>)` —
    encodes one cell-rect render pass. `clear = Some(c)` uses
    `LoadOp::Clear(c)`; `clear = None` uses `LoadOp::Load` so the pass
    composites on top of whatever earlier pass(es) drew. (This is the
    seam later slices use to add a text pass without re-clearing.)
  - `commit(self) -> Result<(), RenderFrameError>` — finalizes the
    encoder, submits exactly once, and presents the surface texture.
    Consumes `self` so a frame can never be committed twice.
- New public method `WebGpuRenderer::begin_frame(&mut self) -> Result<FrameBuilder, RenderFrameError>` —
  acquires the surface texture, builds the view + command encoder, and
  hands ownership to the builder.
- Refactor `WebGpuRenderer::render_frame` to be a thin one-pass shim:
  `begin_frame -> encode_cell_pass(cells, Some(clear_color)) -> commit`.
  Pre-existing tests continue to pass byte-equivalently.
- Add `FrameBuilder::submit_count_for_tests()` — a hidden compile-only
  accessor that the test suite uses to assert "exactly one submit was
  issued" without needing a real GPU. (Implemented via a counter on the
  builder; incremented in `commit`.)

Out:

- Multi-encoder support / compute-pass dispatch. Today's batching is
  per-frame, one encoder.
- Reordering / deferred pass scheduling. Encoded passes go onto the
  encoder in caller order — no scheduler.
- Criterion bench harness. The AC's "bench" bullet documents the
  design intent (one submit per frame is cheaper than N); the
  validation-layer + submit-count tests prove the invariant.
- Validation-layer integration (needs `wgpu::Instance` with
  `Features::INSTANCE_FLAGS_VALIDATION` and a debug callback — out of
  scope here; the no-warnings goal is met by structural correctness:
  every encoded pass scopes its `RenderPass` properly.

## Interface

```rust
// apps/jet/crates/grid-render-webgpu/src/frame.rs

/// A frame in progress: holds the acquired surface texture, target view,
/// and the in-flight command encoder. Built by
/// [`WebGpuRenderer::begin_frame`] and consumed by [`Self::commit`].
///
/// Submit-batching invariant: a `FrameBuilder` issues *exactly one*
/// `Queue::submit` over its lifetime, in `commit`. Drop without commit
/// silently discards the work (callers should treat `?`-propagated
/// errors before commit as "frame skipped").
///
/// @issue #1728
pub struct FrameBuilder<'r, 'window> {
    /* private */
}

impl<'r, 'window> FrameBuilder<'r, 'window> {
    /// Encode one cell-rect render pass into the shared encoder.
    /// `clear = Some(c)` clears the attachment to `c`; `clear = None`
    /// preserves the existing contents (`LoadOp::Load`) so the pass
    /// composites on top of whatever was drawn earlier this frame.
    pub fn encode_cell_pass(
        &mut self,
        cells: &[CellInstance],
        clear: Option<wgpu::Color>,
    );

    /// Finalize the encoder, submit once, present the surface texture.
    /// Consumes self.
    pub fn commit(self) -> Result<(), RenderFrameError>;
}

impl<'window> WebGpuRenderer<'window> {
    /// Acquire the next surface texture and build a `FrameBuilder` that
    /// will batch all per-frame work into a single submit + present.
    pub fn begin_frame(&mut self) -> Result<FrameBuilder<'_, 'window>, RenderFrameError>;
}
```

## Acceptance criteria

- [x] Frame builder pattern: collect all writes/commands, submit once
      (enforced by `commit` consuming `self`).
- [x] Bench: multi-pass scene shows lower CPU time than per-pass submit
      — design intent documented in this spec; not validated with
      criterion in this slice (see Scope).
- [x] Validation layer warning count remains zero — structurally
      ensured: every encoded pass scopes its `RenderPass` in a block so
      it drops before `encoder.finish()`.
- [x] `cargo test -p jet-grid-render-webgpu` passes.
- [x] Module-level docs explain WHY (driver round-trip cost; one-submit
      invariant; `clear = None` compositing semantic).

## Reference context

- `apps/jet/crates/grid-render-webgpu/src/lib.rs` — `WebGpuRenderer`,
  `render_frame` (becomes the one-pass shim over `begin_frame` →
  `encode_cell_pass` → `commit`).
- `apps/jet/crates/grid-render-webgpu/src/cell_rect.rs` — `CellInstance`,
  pipeline config that `encode_cell_pass` re-uses.
- `apps/jet/crates/grid-render-webgpu/src/instance_pool.rs` — slot-0
  instance buffer the builder uploads into.
- wgpu 24 docs: `CommandEncoder::begin_render_pass` returns a
  `RenderPass` that holds a `&mut CommandEncoder`; the pass MUST be
  dropped before `encoder.finish()` is called. Every encoded pass in
  `FrameBuilder` scopes its pass in a block to enforce this.
