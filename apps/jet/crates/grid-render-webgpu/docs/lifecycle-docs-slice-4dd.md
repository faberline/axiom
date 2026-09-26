# Lifecycle docs + idiomatic init example — Slice 4dd

> Issue: #1748 · Parent epic: #1254 · Slice: 4dd

## Problem

Slices 4a–4cc landed the full render runtime (`WebGpuRenderer`,
headless smoke harness, wasm bridge, resize observer hookup, GPU
memory accounting, alpha + present mode pickers, screenshots…)
but the crate has no single doc page or runnable example that
walks a new caller through the canonical lifecycle. Downstream
consumers (`jet-grid`, `jet-grid-wasm`, `cue`) each rolled
their own boot sequence by reading source, which is brittle:
when the next refactor reshuffles `new` / `with_alpha_mode` /
`on_resize_logical` / `render_frame`, every consumer drifts.

This slice closes that gap by:

1. **Extending the `lib.rs` module docs** with a Lifecycle section
   that names the four phases (Construct → Configure → Render →
   Teardown) and the public entry points for each. The docs
   explain the *invariants* the caller signs up to — not a
   restatement of method signatures (rustdoc already gives that).
2. **Adding `examples/minimal.rs`** — a `cargo run --example minimal`
   target that drives one rendered frame against the headless
   adapter (`headless::request_smoke_adapter` →
   `HeadlessSmokeRenderer::render_single_cell`). The example
   gracefully degrades to a skip-with-message when no adapter is
   reachable (typical CI worker without a software fallback),
   mirroring the integration test's skip semantics from Slice 4t.

The headless path is the right target because (a) it runs on
every developer machine + CI box without a window system,
(b) it covers the same pipeline that the surface-bound renderer
exercises, and (c) the harness is already public.

## Scope

In:

- New `examples/minimal.rs` at the crate root.
  - Uses `pollster::block_on` to drive the async adapter +
    renderer construction without pulling tokio.
  - Calls `headless::request_smoke_adapter()`; on `None`, prints
    a skip message and exits `0` (matches the integration-test
    skip contract).
  - On adapter present: builds a `HeadlessSmokeRenderer` at a
    fixed 64×64 size, renders one `CellInstance` (single coloured
    rect on a white clear), and prints
    `rendered minimal frame: 64x64, NNNN bytes` so the user sees a
    concrete signal that the pipeline executed.
  - Annotated with `//! @spec` and `//! @issue` headers.
- Module-level docs extension in `apps/jet/crates/grid-render-webgpu/src/lib.rs`:
  - A `# Lifecycle` section under the existing crate doc, naming the
    four phases and the public entry points for each.
  - A `# Example` section that points readers at
    `examples/minimal.rs` (the *runnable* version) — the doc
    itself stays prose so future renames don't break a doctest.
  - All additions are **doc-only** to the existing top-of-file
    `//!` block. No new public items.
- `pollster` already lives in `[dev-dependencies]` for the
  `#[ignore]`'d live-GPU tests; the example reuses it without a
  new dep.

Out:

- Doctest with executable WebGpu code. wgpu adapter acquisition
  needs `async` + network of GPU drivers we don't control — a
  doctest that gates on the adapter being present is noise. The
  runnable example is the right venue.
- A wasm32 variant of the example. The wasm32 path is exercised
  by the existing `jet-grid-wasm` bridge + its tests; the
  `minimal` example targets native callers (the docs explicitly
  note this).
- Refactors to existing API. Doc-only + example-only changes.
- Public-API surface additions. The AC bullet
  "No new public API surface beyond what existed before this
  slice" is preserved.

## Interface

No new public API. The example uses already-public entry points:

```rust
// Pseudocode of the example's call sequence — see examples/minimal.rs.
let (_instance, adapter) =
    headless::request_smoke_adapter().await
    .expect_or_skip("no wgpu adapter available");

let mut renderer = HeadlessSmokeRenderer::new(adapter, (64, 64)).await?;

let cell = CellInstance {
    pos_px:  [8.0, 8.0],
    size_px: [48.0, 48.0],
    color:   [0.2, 0.6, 0.9, 1.0],
};
let pixels = renderer.render_single_cell(cell, [1.0; 4]).await?;
println!("rendered minimal frame: {}x{}, {} bytes",
    renderer.size_px().0, renderer.size_px().1, pixels.len());
```

The lifecycle doc additions land in `lib.rs` as additional
`//!` lines before the `use` block; they document but do not
change behaviour.

## Acceptance Criteria

- [x] Module docs in lib.rs document the full lifecycle (Construct,
      Configure, Render, Teardown) with the canonical entry
      points named for each phase.
- [x] Example in `examples/minimal.rs` runs a single rendered frame.
- [x] `cargo run --example minimal` succeeds against the headless
      adapter (or prints a skip message + exits 0 when no adapter
      is reachable — same contract as the Slice 4t integration test).
- [x] No new public API surface beyond what existed before this slice.
- [x] `cargo test -p jet-grid-render-webgpu` passes.
- [x] Module-level docs explain the WHY: each phase calls out the
      invariant the caller must honour (surface + config reconfigure
      together; viewport seeded before first draw; `destroy()` is
      observable teardown not extra cleanup), not just the what.

## Reference Context

- `apps/jet/crates/grid-render-webgpu/src/lib.rs` — host of the
  Lifecycle docs extension. The four phases map to:
  - **Construct**: `WebGpuRenderer::new` /
    `WebGpuRenderer::with_alpha_mode` (`lib.rs:176` / `lib.rs:196`).
  - **Configure**: `on_resize` / `on_resize_logical` /
    `set_dpr` / `set_present_mode` / `set_alpha_mode` /
    `set_msaa_count` / `set_clear_color`.
  - **Render**: `render_frame` / `render_frame_clipped` /
    `begin_frame` / `try_acquire_frame` (the four entry points
    that drive a frame).
  - **Teardown**: implicit `Drop`; on wasm, `RendererHandle::destroy`
    is the JS-observable teardown point.
- `apps/jet/crates/grid-render-webgpu/src/headless.rs` — the
  headless smoke renderer the example drives.
- `apps/jet/crates/grid-render-webgpu/tests/headless_smoke.rs` — the
  Slice 4t integration test that pioneered the skip-when-no-adapter
  contract the example mirrors.
- Parent epic #1254 — WebGPU-React renderer. Slice 4dd is the
  documentation capstone over the foundation slices 4a–4cc.
