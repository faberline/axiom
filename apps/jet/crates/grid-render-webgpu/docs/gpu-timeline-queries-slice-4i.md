# GPU timeline queries — frame timing — Slice 4i

> **Issue**: #1727
> **Parent epic**: #1254 (WebGPU-React renderer)
> **Depends on**: #1719–#1723 (Slices 4a–4e — renderer + `render_frame`),
> #1728 (Slice 4j — submit batching / `FrameBuilder`).
> **Status**: in-flight

## Problem

The devtools frame-timing overlay (parent epic's `#devtools` track)
needs per-pass GPU timing for the cell-rect pass: how long does the GPU
spend executing the work `FrameBuilder` submits, not how long the CPU
spends encoding it. The wgpu primitive for this is `QuerySet` +
`PassTimestampWrites` — write a `BEGIN` timestamp when a pass starts, an
`END` timestamp when it ends, `resolve_query_set` into a GPU buffer,
`copy_buffer_to_buffer` into a CPU-mappable readback, `map_async` to
pull the result back one frame later.

Two complications shape the design:

1. **Optional feature.** `Features::TIMESTAMP_QUERY` is not guaranteed
   on every adapter (Firefox WebGPU today, some older drivers). The
   pool must degrade silently — `last_frame_gpu_ms()` returns `None`
   forever instead of failing renderer construction.
2. **One-frame latency.** A timestamp written on frame N is not safe to
   map until frame N's submission has actually completed on the GPU.
   The standard idiom is a ring buffer of N slots; frame N writes slot
   `N % capacity`, and we map the *oldest* slot to read the result of
   the frame that just finished.

## Scope

In:

- New module `frame_timing` exposing `FrameTimingPool`:
  - `FrameTimingPool::new(device, queue, adapter_features)` — if
    `TIMESTAMP_QUERY` is in `adapter_features`, allocates a 2-slot ring
    of `QuerySet(2)` + resolve buffer + readback buffer. Otherwise
    returns a permanently-disabled pool whose every method is a no-op.
  - `FrameTimingPool::is_enabled() -> bool` — for tests and overlays.
  - `FrameTimingPool::last_frame_gpu_ms() -> Option<f32>` — returns the
    last successfully-mapped GPU duration in milliseconds, or `None` if
    no frame has completed yet or the pool is disabled.
  - Internal slot-rotation methods called by `FrameBuilder` on `commit`.
- `WebGpuRenderer::last_frame_gpu_ms() -> Option<f32>` — public delegate
  to the pool. The devtools overlay (later slice) reads through this.
- `WebGpuRenderer::new` now opts into `Features::TIMESTAMP_QUERY`
  *only if the adapter supports it* (probe `adapter.features()` first).
  No adapter requirement change for hosts without the feature.
- `FrameBuilder::encode_cell_pass` wires `timestamp_writes` on the
  `RenderPassDescriptor` when the pool is enabled and a slot is
  available.
- `FrameBuilder::commit` resolves the current slot's queries, copies
  into the readback buffer, kicks off `map_async` for this slot, and
  polls the device non-blocking so previously-submitted slots' map
  callbacks can fire (writing into the shared result cell that
  `last_frame_gpu_ms` observes).

Out:

- Multi-pass timing aggregation (separate timestamps for text/overlay
  passes). Today's pool times the cell pass only; later slices can
  extend `acquire_slot` to return multiple `(begin, end)` index pairs.
- CPU-frame-time measurement. Already feasible via the JS RAF loop in
  `jet-grid-wasm`; out of scope here.
- Histogram / rolling-average output. The pool reports the *latest*
  frame's ms; overlay code can compute its own rolling stats.
- A real-GPU smoke test — needs an adapter with `TIMESTAMP_QUERY`
  support and a polling driver. Covered structurally: feature gate
  guards every wgpu call, disabled pool is exercised by host-target
  tests.

## Interface

```rust
// apps/jet/crates/grid-render-webgpu/src/frame_timing.rs

/// Per-frame GPU timestamp ring buffer. Owns N slots, each a 2-query
/// `QuerySet` + a resolve buffer + a CPU-mappable readback buffer.
/// Disabled (no-op) when the adapter does not expose
/// `Features::TIMESTAMP_QUERY`.
///
/// @issue #1727
pub struct FrameTimingPool { /* private */ }

impl FrameTimingPool {
    /// Build a pool. If `TIMESTAMP_QUERY` is missing from
    /// `adapter_features`, returns a permanently-disabled pool.
    pub(crate) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        adapter_features: wgpu::Features,
    ) -> Self;

    /// `true` if the pool is wired against a real `TIMESTAMP_QUERY`
    /// adapter feature; `false` for the no-op fallback.
    pub fn is_enabled(&self) -> bool;

    /// Most recently completed frame's GPU duration in milliseconds.
    /// `None` if no frame has completed yet, or the pool is disabled.
    pub fn last_frame_gpu_ms(&self) -> Option<f32>;
}

impl<'window> WebGpuRenderer<'window> {
    /// Convenience delegate to the internal `FrameTimingPool`.
    pub fn last_frame_gpu_ms(&self) -> Option<f32>;
}
```

## Acceptance criteria

- [x] QuerySet allocated per-frame slot (ring buffer of capacity 2,
      allocated up front in `FrameTimingPool::new`).
- [x] Timestamp writes at pass begin/end (wired via
      `RenderPassDescriptor::timestamp_writes` in
      `FrameBuilder::encode_cell_pass`).
- [x] Map result buffer back to CPU one frame later (`map_async` kicked
      off in `FrameBuilder::commit`, callback writes the shared result
      cell; `Device::poll(Maintain::Poll)` drives previous frames'
      callbacks).
- [x] Expose latest-frame ms via
      `WebGpuRenderer::last_frame_gpu_ms() -> Option<f32>` (returns
      `None` when feature is unavailable or no frame has completed).
- [x] `cargo test -p jet-grid-render-webgpu` passes.
- [x] Module-level docs explain WHY (optional-feature degradation +
      one-frame map-back latency invariant).

## Reference context

- `apps/jet/crates/grid-render-webgpu/src/lib.rs` — `WebGpuRenderer::new`
  (where we probe `adapter.features()` for `TIMESTAMP_QUERY` and
  conditionally add it to `required_features`).
- `apps/jet/crates/grid-render-webgpu/src/frame.rs` — `FrameBuilder`
  (where `encode_cell_pass` and `commit` wire timestamp writes /
  resolve / map-back).
- `apps/jet/crates/grid-render-webgpu/docs/submit-batching-slice-4j.md` —
  one-submit-per-frame invariant. Timing queries must NOT add a second
  submit; they ride on the same encoder.
- wgpu 24 docs:
  - `QuerySetDescriptor { ty: QueryType::Timestamp, count: 2 }` per slot.
  - `RenderPassTimestampWrites { query_set, beginning_of_pass_write_index,
    end_of_pass_write_index }`.
  - `CommandEncoder::resolve_query_set` writes raw u64 ticks into a
    `BufferUsages::QUERY_RESOLVE | COPY_SRC` buffer.
  - `Queue::get_timestamp_period()` returns ns per tick.
  - `Buffer::map_async` + `Device::poll(Maintain::Poll)` for non-blocking
    readback.
