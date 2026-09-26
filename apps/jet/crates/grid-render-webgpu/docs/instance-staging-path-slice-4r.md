# Instance buffer staging path selection — Slice 4r

> Issue: #1736 · Parent epic: #1254 · Slice: 4r

## Problem

`InstanceBufferPool::get_or_grow` uploads cell-instance payloads via
`Queue::write_buffer` unconditionally. That API is the cheap path for
small payloads: wgpu allocates a slice from a per-queue staging arena,
memcpys the caller's `&[u8]` into the staging slice, then schedules a
copy_buffer_to_buffer at the next submit.

The user-slice → staging memcpy is the cost. For payloads above a few
megabytes it shows up as visible CPU time per frame on profiles. wgpu
exposes a "write directly into the staging arena" variant —
`Queue::write_buffer_with(buffer, offset, size)` — that returns a
mapped view the caller fills in directly. Same number of GPU copies,
one fewer host-side memcpy.

The renderer needs to:

1. Pick the path by upload size — small payloads keep the current
   `write_buffer` path (its allocator is tuned for small writes; the
   "saved memcpy" win doesn't pay off for tiny slices).
2. Apply a sensible default threshold (64K instances × 32 bytes/instance
   = 2 MiB), configurable so future tuning can move it without code
   change.

## Scope

In:

- New field `staging_threshold_bytes: wgpu::BufferAddress` on
  `InstanceBufferPool`. Default: 2 MiB (`64 * 1024 * 32`). When zero,
  every non-empty upload uses the staged path (escape hatch for tests
  / debugging).
- New free function `pick_staging_path(data_len, threshold) ->
  StagingPath` returning a `Direct | Staged` enum — pure, no GPU,
  unit-testable.
- `get_or_grow` dispatches by `pick_staging_path`: `Direct` →
  `Queue::write_buffer` (today's path); `Staged` →
  `Queue::write_buffer_with` + `copy_from_slice` into the returned
  view, with a graceful fallback to `write_buffer` if the queue
  refuses the staging-arena allocation (rare; happens when payload
  exceeds the internal arena's hard cap).
- New `InstanceBufferPool::with_staging_threshold(bytes)` constructor
  and `set_staging_threshold_bytes(bytes)` setter / getter pair.
- New `WebGpuRenderer::set_instance_staging_threshold_bytes(bytes)`
  + `instance_staging_threshold_bytes()` getter — public surface so
  callers (devtools, benches) can override at runtime.
- Unit tests on `pick_staging_path` covering: below threshold, at
  threshold (boundary is `>=` → `Staged`), above threshold, empty
  payload, and the `threshold == 0` "stage everything" mode.

Out:

- Benchmarking the actual crossover. The AC's "bench shows crossover
  lands within 10% of per-driver optimum" is a *measurement*
  follow-up, not an implementation invariant; the threshold is a
  ballparked default that a later bench-driven slice can dial in.
  Adding criterion + a GPU host is well out of scope for this slice.
- Manual staging-buffer pattern (allocate a dedicated `MAP_WRITE +
  COPY_SRC` buffer, fill it from a `BufferAsyncSlice`, then
  `encoder.copy_buffer_to_buffer`). That path requires plumbing an
  encoder reference into the pool API and would change every caller.
  `Queue::write_buffer_with` delivers the same "one fewer host memcpy"
  property without an API break.
- Per-payload override at the call site. The pool-level threshold is
  the single tuning knob; if a future caller needs per-buffer control,
  a separate slice can extend the API.

## Interface

```rust
/// Which upload path the pool will take for a payload of `data_len`
/// bytes given the current `staging_threshold_bytes`.
///
/// Pure, no GPU — extracted as a free function so the policy can be
/// unit-tested without an adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StagingPath {
    /// `Queue::write_buffer` — host memcpy into wgpu's staging arena,
    /// then one driver-side copy at submit time.
    Direct,
    /// `Queue::write_buffer_with` — caller writes into the staging
    /// arena view directly, saving the host memcpy.
    Staged,
}

pub fn pick_staging_path(
    data_len: wgpu::BufferAddress,
    threshold: wgpu::BufferAddress,
) -> StagingPath;

impl InstanceBufferPool {
    pub fn with_staging_threshold(bytes: wgpu::BufferAddress) -> Self;
    pub fn staging_threshold_bytes(&self) -> wgpu::BufferAddress;
    pub fn set_staging_threshold_bytes(&mut self, bytes: wgpu::BufferAddress);
}

impl<'window> WebGpuRenderer<'window> {
    /// Override the instance-pool upload-path threshold. `0` forces
    /// the staged path for every non-empty payload.
    pub fn set_instance_staging_threshold_bytes(&mut self, bytes: wgpu::BufferAddress);
    pub fn instance_staging_threshold_bytes(&self) -> wgpu::BufferAddress;
}
```

Threshold-policy contract:

- `data_len == 0` → no upload happens at all (existing skip).
- `data_len > 0 && data_len < threshold` → `Direct`.
- `data_len > 0 && data_len >= threshold` → `Staged`.
- `threshold == 0` → every non-empty payload is `Staged`.

## Acceptance Criteria

- [x] `InstanceBufferPool` picks path by `data.len()` against a
      configurable threshold (default 2 MiB = 64K × 32-byte instances).
- [x] Path-selection logic is unit-tested without a GPU via the pure
      `pick_staging_path` function.
- [x] `Queue::write_buffer_with` failure (returns `None`) falls back
      to `Queue::write_buffer` so a queue-arena exhaustion never drops
      the upload silently.
- [x] `WebGpuRenderer::set_instance_staging_threshold_bytes` exposes
      runtime configurability for future bench-driven tuning.
- [x] `cargo test -p jet-grid-render-webgpu` passes.
- [x] Module-level docs explain the WHY (one-fewer-host-memcpy is the
      win; threshold avoids paying staged-path setup cost on small
      payloads where `write_buffer` is already optimal).

## Reference Context

- `apps/jet/crates/grid-render-webgpu/src/instance_pool.rs:108-153` —
  current `get_or_grow`; this slice dispatches its `queue.write_buffer`
  call site by data length.
- `apps/jet/crates/grid-render-webgpu/src/cell_rect.rs:75-81` —
  `CellInstance` = 32 bytes, the basis for the 64K-instance default.
- `apps/jet/crates/grid-render-webgpu/src/lib.rs:975-984` —
  `WebGpuRenderer::upload_instance_buffer`; passes the bytes straight
  through to the pool, so per-frame upload size is the trigger.
- Sibling Slice 4d (#1722) introduced the pool's grow-no-shrink
  invariant; this slice changes the upload step inside `get_or_grow`
  without touching capacity policy.
