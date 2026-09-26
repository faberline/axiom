# Instance buffer pool for CellInstance uploads — Slice 4d

> **Issue**: #1722
> **Parent epic**: #1254 (WebGPU-React renderer)
> **Depends on**: #1719 (Slice 4a — WebGpuRenderer wrapper),
>                 #1720 (Slice 4b — cell-rect pipeline),
>                 #1721 (Slice 4c — viewport uniform buffer)
> **Status**: in-flight

## Problem

The cell-rect pipeline (Slice 4b) consumes per-instance vertex data through
a `wgpu::Buffer` with `VERTEX | COPY_DST` usage. The naive path would be to
allocate a fresh buffer every frame sized to the current cell count — but
GPU buffer allocation is expensive (driver round-trip, optional zero-fill,
memory fragmentation under churn). Worse, it forces a pipeline pause while
the new buffer is created.

The fix is to maintain a **pool** of growable instance buffers, indexed by
frame slot. Each slot's buffer is sized to the largest payload ever seen in
that slot; subsequent frames at the same slot either reuse the existing
buffer (capacity ≥ demand) or grow it once (capacity < demand). The pool
never shrinks within a session — peak usage is the working set.

The pool is **format-agnostic** in this slice (it only owns byte-sized
buffers). Slices that bind it to a pipeline supply their own typed view
(`bytemuck::cast_slice(...)` of a `[CellInstance]`).

## Scope

Smallest foundation slice that closes the AC bullets:

- New module `instance_pool` exposing `InstanceBufferPool` keyed by slot
  index (`usize`).
- Method `get_or_grow(slot, min_size_bytes, device, queue, data) -> &wgpu::Buffer`
  that either reuses the slot's buffer (capacity ≥ `min_size_bytes`) or
  re-creates it at the next growth tier and uploads `data` via
  `Queue::write_buffer`.
- Growth policy: when a grow is needed, new capacity =
  `max(min_size_bytes, current_capacity * 3 / 2)` rounded up to a 16-byte
  boundary. The factor `1.5×` matches the AC bullet; the 16-byte rounding
  matches WebGPU's COPY alignment requirement and avoids degenerate single-
  byte grows when callers feed tiny payloads.
- Initial capacity: `0` (no buffer allocated until first `get_or_grow`).
- No shrink path. The pool grows monotonically per slot for the lifetime of
  the `InstanceBufferPool` value.

**Out of scope** for this slice:

- Hooking the pool into `WebGpuRenderer` as a field — the pool is a free-
  standing utility this slice. The next slice (frame-loop orchestrator)
  will wire it onto the renderer.
- Multi-frame-in-flight ordering / fences. The pool is just a sized-buffer
  cache; frame-pacing is a separate concern.
- Sub-allocation within a single buffer. The pool model is one buffer per
  slot, not a bump allocator.

## Interface

```rust
// apps/jet/crates/grid-render-webgpu/src/instance_pool.rs

/// Growable instance-buffer pool, indexed by frame slot.
///
/// Each slot's buffer carries `wgpu::BufferUsages::VERTEX | COPY_DST`.
/// Grows monotonically — never shrinks within a session.
///
/// @issue #1722
pub struct InstanceBufferPool {
    slots: Vec<Option<PoolSlot>>,
}

struct PoolSlot {
    buffer: wgpu::Buffer,
    capacity: wgpu::BufferAddress,
}

impl InstanceBufferPool {
    /// Construct an empty pool. Allocation is deferred until the first
    /// `get_or_grow` call so embed-then-never-use is free.
    pub fn new() -> Self;

    /// Number of slots the pool currently tracks (peak slot index + 1, or
    /// 0 if no slots have been touched).
    pub fn len(&self) -> usize;

    /// `true` iff no slot has ever been touched.
    pub fn is_empty(&self) -> bool;

    /// Reuse-or-grow the buffer at `slot` so it can hold at least
    /// `min_size_bytes` of payload, then upload `data` (which MUST be
    /// `<= min_size_bytes` long). Returns a reference to the underlying
    /// buffer for binding.
    ///
    /// Growth policy: if capacity < min_size_bytes, new capacity =
    /// `max(min_size_bytes, capacity * 3 / 2)` rounded up to 16 bytes.
    /// If capacity >= min_size_bytes, the existing buffer is reused (no
    /// realloc) and `data` is just written into it.
    ///
    /// Panics if `data.len() > min_size_bytes` — that's a caller bug.
    /// Panics if `min_size_bytes == 0` — there is no use case for an
    /// empty allocation.
    pub fn get_or_grow(
        &mut self,
        slot: usize,
        min_size_bytes: wgpu::BufferAddress,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        data: &[u8],
    ) -> &wgpu::Buffer;

    /// Current capacity (bytes) at `slot`, or 0 if the slot has never
    /// been touched. Primarily for tests that want to assert the
    /// no-shrink invariant.
    pub fn capacity(&self, slot: usize) -> wgpu::BufferAddress;
}
```

## Acceptance criteria

- [x] `BufferPool` with `get_or_grow(min_size) -> &Buffer` — see
      `InstanceBufferPool::get_or_grow` above. (Parameter is
      `min_size_bytes` for explicitness.)
- [x] Each pool slot owns one `wgpu::Buffer` with
      `USAGE_VERTEX | COPY_DST`.
- [x] Growth policy: `1.5×` current capacity when exceeded (rounded up to
      16 bytes; `min_size_bytes` floor when the new demand outpaces 1.5×).
- [x] Tests cover: first allocation, growth, no-shrink invariant.
- [x] `cargo test` passes for `jet-grid-render-webgpu`.
- [x] Module-level docs explain the WHY (why the pool exists, growth
      policy, no-shrink invariant).

## Reference context

- `apps/jet/crates/grid-render-webgpu/src/lib.rs` — Slice 4a wrapper; pattern
  for owning per-renderer GPU resources, persistent buffer + COPY_DST.
- `apps/jet/crates/grid-render-webgpu/src/cell_rect.rs` — Slice 4b; defines
  the `CellInstance` byte layout the pool will eventually carry.
- wgpu 24 docs: `Queue::write_buffer` requires the destination buffer to
  have `COPY_DST` usage and the offset+size to be 4-byte-aligned. The
  pool rounds capacity to 16 bytes which trivially satisfies this.
- `bytemuck::cast_slice` is the canonical bridge from `&[CellInstance]` to
  `&[u8]`; callers pass the cast result as `data`.
