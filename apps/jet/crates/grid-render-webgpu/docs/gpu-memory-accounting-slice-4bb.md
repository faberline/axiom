# GPU memory accounting — Slice 4bb

> Issue: #1746 · Parent epic: #1254 · Slice: 4bb

## Problem

The renderer holds GPU resources whose total footprint scales with the
surface size, MSAA count, and how many instance-buffer slots the
batching layer has grown into. Two downstream consumers need this
number:

- **Devtools (Epic 17)** — a "GPU memory used by the grid"
  number rendered in the overlay panel. Operators reading this in
  production want a single integer (bytes), not the resource-by-
  resource breakdown.
- **Eviction policy (Epic 13)** — the text-atlas eviction policy
  needs to know how close the renderer is to a budget so it can shed
  glyph pages.

A precise byte-accurate counter would require instrumenting every
`Device::create_*` call site (textures, buffers, pipelines, query
sets, …). That's invasive *and* not actually what the consumers
need: an **approximate** snapshot computed from the renderer's
already-tracked state (surface dims, format, MSAA count, instance
pool capacities) is enough. The approximation is conservative —
small fixed-size allocations (16-byte viewport uniform, the timing
pool's 8-byte query buffers) get an explicit floor; pipeline /
shader-module / bind-group state-object memory is excluded because
it isn't surface-scaled and dominates only on heavily-multi-format
adapters (not the grid path).

This slice (1) adds a public report type, (2) adds a derive-from-
state method that snapshots the current footprint, and (3) exposes
pure helpers (`texture_bytes`, `bytes_per_pixel`) so callers can
predict the next allocation's cost without going through the
renderer.

## Scope

In:

- New module `gpu_memory` with:
  - `pub struct GpuMemoryReport { pub textures: u64, pub buffers: u64, pub atlas: u64, pub total: u64 }` —
    plain-old-data, all `u64` bytes. The four fields match the AC's
    `{ textures, buffers, atlas, total }`. `atlas` is a placeholder
    for the text-glyph atlas (Slice 5a+); the field is reserved
    here so devtools doesn't have to ship a schema update later.
  - `GpuMemoryReport::new(textures, buffers, atlas)` constructor
    that computes `total = textures + buffers + atlas`. Saturating
    arithmetic — overflow saturates to `u64::MAX` (defensive — real
    GPU memory is bounded by GPU VRAM, but the report sums are
    summed across slots and could theoretically overflow on a
    pathological caller).
  - `pub fn bytes_per_pixel(format: wgpu::TextureFormat) -> u32` —
    wraps `wgpu::TextureFormat::block_copy_size(None)`. Returns 0
    for compressed / depth / stencil formats that don't have a
    well-defined per-pixel size (we don't allocate those in the
    grid path; this is a defensive guard for future callers).
  - `pub fn texture_bytes(width: u32, height: u32, format: wgpu::TextureFormat) -> u64` —
    `width as u64 * height as u64 * bytes_per_pixel(format) as u64`.
    Pure, deterministic, no `Device` required.
- New method `WebGpuRenderer::gpu_memory_estimate() -> GpuMemoryReport`:
  - **textures** = `texture_bytes(surface_width, surface_height, format)` +
    (if MSAA active) `texture_bytes(surface_width, surface_height, format) * msaa_count as u64`.
    The MSAA-side approximation treats the multisampled color texture as
    `samples × backbuffer_bytes`, which is what wgpu / Metal / Vulkan
    actually allocate internally.
  - **buffers** = viewport uniform (16 bytes) + sum of
    `instance_pool.capacity(slot)` over `0..instance_pool.len()` +
    `frame_timing.buffer_bytes_estimate()` (8 bytes per slot × pool
    capacity — `FrameTimingPool` already exposes its slot count).
  - **atlas** = 0 (no atlas allocated by this crate yet — closed
    over in a later slice).
- Module-level doc on `gpu_memory` explaining the WHY: report is
  *approximate* by design (snapshot of tracked state, not a
  hooked-allocator counter), and consumers (devtools, eviction
  policy) only need order-of-magnitude accuracy.

Out:

- Push-based instrumentation of every `Device::create_*` call site.
  The AC says "updated on every allocate / drop" — that's
  satisfied by the derive-from-state model: every allocation
  changes the field we read from (surface dims, MSAA state,
  instance-pool slot count), so the next call to
  `gpu_memory_estimate()` reflects it. No event hooks needed.
- Pipeline / shader-module / bind-group state-object bytes. Not
  surface-scaled, dominate only on multi-format adapters (not this
  crate), and wgpu doesn't expose a per-pipeline-size accessor.
- The actual text atlas accounting. The `atlas` field is reserved
  for Slice 5a+; this slice ships it as `0`.
- Live integration test that measures real driver allocation. wgpu
  doesn't expose driver-side memory; the closest signal is the
  `Adapter::get_info` heap reports, which are coarse and platform-
  variable. The unit tests assert the *formula* against the AC's
  256×256 RGBA8 = 256 KiB anchor.

## Interface

```rust
/// Approximate GPU memory footprint of this renderer's allocations.
/// All values are bytes; the report is a *snapshot* computed from
/// the renderer's tracked state, not a hooked-allocator counter.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/gpu-memory-accounting-slice-4bb.md#interface
/// @issue #1746
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GpuMemoryReport {
    /// Total bytes attributable to textures (surface backbuffer +
    /// MSAA color, when enabled).
    pub textures: u64,
    /// Total bytes attributable to buffers (uniforms, instance
    /// pool slots, timing-pool readback buffers).
    pub buffers: u64,
    /// Total bytes attributable to the glyph atlas. `0` until
    /// Slice 5a wires the text path.
    pub atlas: u64,
    /// Convenience sum: `textures + buffers + atlas`. Saturating
    /// `u64::MAX` on overflow (defensive).
    pub total: u64,
}

impl GpuMemoryReport {
    pub fn new(textures: u64, buffers: u64, atlas: u64) -> Self;
}

/// Per-pixel byte size for `format`. Wraps
/// `wgpu::TextureFormat::block_copy_size(None)`; returns 0 for
/// compressed / depth / stencil formats (defensive — not used by the
/// grid path).
pub fn bytes_per_pixel(format: wgpu::TextureFormat) -> u32;

/// Bytes for a `width × height` texture of `format`: pure
/// `width * height * bytes_per_pixel(format)`. Caller-callable
/// without a `Device`.
pub fn texture_bytes(width: u32, height: u32, format: wgpu::TextureFormat) -> u64;

impl<'window> WebGpuRenderer<'window> {
    /// Snapshot the renderer's approximate GPU footprint. Cheap —
    /// reads tracked fields, runs no GPU command.
    pub fn gpu_memory_estimate(&self) -> GpuMemoryReport;
}
```

## Acceptance Criteria

- [x] `WebGpuRenderer::gpu_memory_estimate() -> GpuMemoryReport { textures, buffers, atlas, total }`
      — public method, shape matches the AC verbatim.
- [x] Updated on every allocate / drop — derive-from-state model:
      `gpu_memory_estimate` reads `surface_config`, `msaa_count`,
      `instance_pool.len()` / `capacity(slot)`, and
      `frame_timing.buffer_bytes_estimate()`. Each allocation/drop
      mutates one of those fields, so the next call to
      `gpu_memory_estimate` reflects the change. No hook plumbing.
- [x] Test: allocating a 256×256 RGBA8 texture increases `textures`
      by 256 KiB — covered by the pure-fn test
      `texture_bytes_256x256_rgba8_is_256kib` asserting
      `texture_bytes(256, 256, Rgba8Unorm) == 256 * 1024`. The
      formula is the same the renderer's estimate uses.
- [x] `cargo test -p jet-grid-render-webgpu` passes.
- [x] Module-level doc explains the WHY: report is approximate
      by design, consumers (devtools, eviction) only need
      order-of-magnitude accuracy.

## Reference Context

- `apps/jet/crates/grid-render-webgpu/src/instance_pool.rs` —
  `InstanceBufferPool::{len, capacity}` already expose the bytes
  per slot; `gpu_memory_estimate` consumes those directly.
- `apps/jet/crates/grid-render-webgpu/src/frame_timing.rs` — adds a
  `buffer_bytes_estimate()` accessor (matches the existing
  `is_enabled` / `active_query_set` style).
- `apps/jet/crates/grid-render-webgpu/src/lib.rs` — `surface_config`,
  `msaa_count`, `msaa_color`, `viewport_buffer`, `instance_pool`,
  `frame_timing` are the read-only sources of truth.
- Parent epic #1254 — WebGPU-React renderer; Slice 4bb closes the
  memory-accounting gap so devtools (#1740-class slices) and
  eviction-policy (Epic 13) consumers can read a single number.
