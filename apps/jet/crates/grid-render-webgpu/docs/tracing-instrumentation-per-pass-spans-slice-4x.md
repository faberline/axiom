# Tracing instrumentation — per-pass spans — Slice 4x

> Issue: #1742 · Parent epic: #1254 · Slice: 4x

## Problem

Every frame the renderer does several measurable units of work:
acquire the surface, encode the cell-rect pass, (in future slices)
encode the text pass, finish the encoder, submit + present. Today
none of these are visible to a profiler — `chrome://tracing`, Tracy,
and the future devtools overlay all want a structured `tracing` span
tree so a human can answer "where did this 16 ms go?" without
instrumenting by hand each session.

This slice wraps the existing render path in `tracing` spans:

- A **frame** span around the full `render_frame` / `commit` cycle
  (so a Tracy timeline shows one block per frame).
- A **pass** span nested inside the frame for the cell-rect render
  pass; future slices add `text_pass` and other passes inside the
  same frame span.
- **Fields** on the spans so post-hoc analysis can pivot on cell
  count and (in the future) glyph count: `cells = N` on the cell
  pass, `glyphs = N` on the text pass when it lands.

The renderer crate doesn't own application-level subscriber wiring
(an app's main typically calls `tracing_subscriber::fmt()` itself).
But the AC asks for `RUST_LOG` support — so this slice also exposes
a thin helper `install_default_subscriber()` that the JS bridge,
tests, and CLI binaries can call once to get a fmt subscriber
configured from `RUST_LOG` via `EnvFilter::from_default_env()`. The
helper is idempotent at *call* level (returns `Err` if a global
subscriber is already installed), so tests can drive it without
breaking each other.

## Scope

In:

- New dep `tracing-subscriber = { workspace = true, features = ["env-filter"] }`.
- New `pub fn install_default_subscriber() -> Result<(), SubscriberInstallError>`
  in a new `pub mod tracing_setup` — wraps `tracing_subscriber::fmt`
  + `EnvFilter::from_default_env`. `SubscriberInstallError` is a
  thin newtype around `tracing_subscriber::util::TryInitError` (or
  equivalent) so callers don't take a direct dep on the subscriber
  crate.
- A top-level `tracing::info_span!("frame", ...)` span in
  `WebGpuRenderer::render_frame` around `begin_frame`,
  `encode_cell_pass`, and `commit`. Span fields:
  `cells = cells.len()` and `frame_id = atomic_u64` (monotonic, for
  ordering in chrome://tracing).
- A nested `tracing::info_span!("cell_pass", cells = ...)` in
  `FrameBuilder::encode_cell_pass`. The pass span is a *child* of
  the current frame span (tracing's parent-resolution rules — the
  frame span is the active span when `encode_cell_pass` runs).
- A reserved `glyphs` field name documented for the future
  `text_pass` span (out of this slice's implementation scope —
  documented in the spec so the next slice that adds the text pass
  knows the agreed field name).
- Module-level doc on `tracing_setup` explaining the WHY:
  application-level subscribers are app concerns, but a thin shared
  helper lets the JS bridge, headless tests, and CLI bins drop into
  a uniform default without each duplicating the wiring.
- Unit tests:
  - `install_default_subscriber_is_callable` — calls the helper and
    asserts it returns `Ok` or `Err(AlreadyInstalled)` (test ordering
    is non-deterministic, so either is acceptable as long as the
    type is right).
  - `frame_id_counter_is_monotonic` — pumps the internal frame-id
    counter and asserts strict monotonicity.

Out:

- The actual `text_pass` span. The text pass itself doesn't exist
  yet (Slice 4y or later). This slice reserves the `glyphs` field
  name and the `text_pass` span name in the spec.
- Subscriber output formatting (JSON vs. pretty vs. compact). The
  default subscriber uses `fmt()`'s defaults (pretty on TTY, compact
  off-TTY). Callers that want chrome://tracing JSON or Tracy can
  install their own subscriber instead of calling the helper.
- Span event spans (`tracing::trace_span!`). Only `info_span!` is
  added here — trace-level spans add overhead even when disabled
  unless the `tracing` `release_max_level_*` feature gates them.
  Future slices can downgrade per-pass spans to `debug_span!` once
  the perf profile is measured.

## Interface

```rust
pub mod tracing_setup {
    #[derive(Debug, thiserror::Error)]
    pub enum SubscriberInstallError {
        #[error("a global tracing subscriber is already installed")]
        AlreadyInstalled,
    }

    /// Install a `tracing_subscriber::fmt` subscriber with
    /// `EnvFilter::from_default_env()` (reads `RUST_LOG`). Returns
    /// `Err(AlreadyInstalled)` if a global subscriber is already
    /// in place — callers should treat that as a no-op.
    pub fn install_default_subscriber() -> Result<(), SubscriberInstallError>;
}

impl<'window> WebGpuRenderer<'window> {
    // existing render_frame is augmented internally (no signature change):
    pub fn render_frame(
        &mut self,
        cells: &[CellInstance],
    ) -> Result<(), RenderFrameError>;
}
```

Span tree the renderer emits per frame:

```
frame{frame_id=N, cells=M}      // info_span! in render_frame
└── cell_pass{cells=M}          // info_span! in encode_cell_pass
    (future) text_pass{glyphs=K}
```

## Acceptance Criteria

- [x] `tracing::span` at frame begin/end — `info_span!("frame", ...)`
      in `render_frame`. The span's `.enter()` guard covers the
      full frame; the `Drop` of the guard marks frame end.
- [x] Nested spans per pass (cell, text) — `cell_pass` lands in
      this slice; `text_pass` field schema is reserved in the spec
      (the text pass itself is a later slice).
- [x] Span fields include cell count, glyph count — `cells = N` on
      both `frame` and `cell_pass`; `glyphs` field documented for
      the future text pass.
- [x] `tracing-subscriber EnvFilter` respects `RUST_LOG` — via the
      `install_default_subscriber` helper.
- [x] `cargo test -p jet-grid-render-webgpu` passes.
- [x] Module-level docs explain the WHY: the JS bridge + future
      devtools overlay need structured per-pass timing; uniform
      span names + field schema is the seam they read.

## Reference Context

- `apps/jet/crates/grid-render-webgpu/src/lib.rs` — `render_frame`.
- `apps/jet/crates/grid-render-webgpu/src/frame.rs` — `FrameBuilder` +
  `encode_cell_pass`.
- `apps/jet/crates/grid-render-webgpu/src/validation.rs` — existing
  log-bridge install (sibling concern; tracing setup parallels it).
- Parent epic #1254 — WebGPU-React renderer; this slice freezes
  the per-frame tracing schema for the devtools layer.
- `Cargo.toml` line 148 — `tracing-subscriber = "0.3"` workspace
  dep; this slice opts in to the `env-filter` feature.
