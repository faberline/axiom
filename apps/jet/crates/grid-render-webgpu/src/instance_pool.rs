//! Growable instance-buffer pool — one `wgpu::Buffer` per frame slot,
//! sized to peak demand.
//!
//! Why this exists: allocating a fresh `wgpu::Buffer` every frame is
//! expensive (driver round-trip, optional zero-fill, fragmentation under
//! churn). The cell-rect pipeline (Slice 4b) wants `VERTEX | COPY_DST`
//! buffers whose size tracks the current cell count — a property best
//! served by reusing a buffer that was already grown to that size on a
//! prior frame.
//!
//! Invariant — no shrink: capacity at each slot grows monotonically for
//! the lifetime of the pool. Peak usage in a slot IS the working set.
//! Callers that change their working set drastically (e.g. switching from
//! a 100k-cell view to a 100-cell view) pay no per-frame cost but keep
//! the larger buffer resident. That's the right trade-off for a render
//! pipeline that re-grows in the other direction milliseconds later.
//!
//! Invariant — growth policy: when demand exceeds capacity, new capacity
//! is `max(min_size_bytes, capacity * 3 / 2)` rounded up to 16 bytes.
//! The 1.5× factor matches the spec; the 16-byte rounding both
//! over-satisfies `Queue::write_buffer`'s 4-byte alignment and avoids
//! degenerate single-byte grows for tiny payloads.
//!
//! Invariant — upload path selection (Slice 4r, #1736): non-empty
//! payloads smaller than `staging_threshold_bytes` use
//! `Queue::write_buffer` (cheap small-write path). Payloads at or
//! above the threshold use `Queue::write_buffer_with`, which returns
//! a view into wgpu's staging arena that the pool fills directly —
//! saving one host-side memcpy that `write_buffer` would otherwise
//! perform from the caller's slice into staging. Same GPU work, less
//! CPU. If `write_buffer_with` returns `None` (queue refuses the
//! staging allocation — rare, hits the arena's hard cap), the pool
//! falls back to `write_buffer` so an upload never silently drops.

/// Default staging-path threshold: 64K cell instances × 32 bytes per
/// `CellInstance` = 2 MiB. The boundary at which `write_buffer_with`
/// begins to outpay its setup cost over `write_buffer` on the integrated
/// GPUs we target. Override via
/// [`InstanceBufferPool::set_staging_threshold_bytes`] for tuning.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/instance-staging-path-slice-4r.md#interface
/// @issue #1736
pub const DEFAULT_STAGING_THRESHOLD_BYTES: wgpu::BufferAddress = 64 * 1024 * 32;

/// Which `wgpu::Queue` upload entry point the pool will reach for on the
/// next `get_or_grow` call given the current payload size and threshold.
///
/// Returned by the pure [`pick_staging_path`] helper so the policy can
/// be unit-tested without a GPU device.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/instance-staging-path-slice-4r.md#interface
/// @issue #1736
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StagingPath {
    /// `Queue::write_buffer` — wgpu memcpys the caller's slice into the
    /// per-queue staging arena, then schedules a `copy_buffer_to_buffer`
    /// at submit. Cheapest for small payloads.
    Direct,
    /// `Queue::write_buffer_with` — the pool fills a view of the
    /// staging arena directly, saving the user-slice → staging memcpy.
    /// Wins for payloads above the threshold (default 2 MiB).
    Staged,
}

/// Pure path-selection rule. `data_len == 0` is never a real upload
/// (caller-side skip), but is canonicalized to `Direct` here for
/// symmetry. `threshold == 0` forces `Staged` for every non-empty
/// payload — the escape hatch for tests / debugging.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/instance-staging-path-slice-4r.md#interface
/// @issue #1736
pub fn pick_staging_path(
    data_len: wgpu::BufferAddress,
    threshold: wgpu::BufferAddress,
) -> StagingPath {
    if data_len == 0 {
        StagingPath::Direct
    } else if data_len >= threshold {
        StagingPath::Staged
    } else {
        StagingPath::Direct
    }
}

/// Growable instance-buffer pool, indexed by frame slot.
///
/// Each slot's buffer carries `wgpu::BufferUsages::VERTEX | COPY_DST`.
/// Capacity grows monotonically — never shrinks within a session.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/instance-buffer-pool-slice-4d.md#interface
/// @issue #1722
pub struct InstanceBufferPool {
    slots: Vec<Option<PoolSlot>>,
    staging_threshold_bytes: wgpu::BufferAddress,
}

struct PoolSlot {
    buffer: wgpu::Buffer,
    capacity: wgpu::BufferAddress,
}

impl Default for InstanceBufferPool {
    fn default() -> Self {
        Self::new()
    }
}

impl InstanceBufferPool {
    /// Construct an empty pool. Allocation is deferred until the first
    /// [`get_or_grow`](Self::get_or_grow) call so a pool that is never
    /// touched costs nothing on the GPU.
    ///
    /// The staging-path threshold defaults to
    /// [`DEFAULT_STAGING_THRESHOLD_BYTES`]; use
    /// [`Self::with_staging_threshold`] or
    /// [`Self::set_staging_threshold_bytes`] to override.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/instance-buffer-pool-slice-4d.md#interface
    /// @issue #1722
    pub fn new() -> Self {
        Self {
            slots: Vec::new(),
            staging_threshold_bytes: DEFAULT_STAGING_THRESHOLD_BYTES,
        }
    }

    /// Construct an empty pool with a custom staging-path threshold.
    /// `bytes == 0` forces the staged path for every non-empty upload —
    /// useful for tests / debugging.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/instance-staging-path-slice-4r.md#interface
    /// @issue #1736
    pub fn with_staging_threshold(bytes: wgpu::BufferAddress) -> Self {
        Self {
            slots: Vec::new(),
            staging_threshold_bytes: bytes,
        }
    }

    /// Current staging-path threshold in bytes. See
    /// [`pick_staging_path`] for the decision rule.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/instance-staging-path-slice-4r.md#interface
    /// @issue #1736
    pub fn staging_threshold_bytes(&self) -> wgpu::BufferAddress {
        self.staging_threshold_bytes
    }

    /// Replace the staging-path threshold. Takes effect on the next
    /// [`get_or_grow`](Self::get_or_grow) call; in-flight uploads are
    /// unaffected.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/instance-staging-path-slice-4r.md#interface
    /// @issue #1736
    pub fn set_staging_threshold_bytes(&mut self, bytes: wgpu::BufferAddress) {
        self.staging_threshold_bytes = bytes;
    }

    /// Number of slots the pool currently tracks (peak slot index + 1, or
    /// 0 if no slot has been touched).
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/instance-buffer-pool-slice-4d.md#interface
    /// @issue #1722
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    /// `true` iff no slot has ever been touched.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/instance-buffer-pool-slice-4d.md#interface
    /// @issue #1722
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    /// Current capacity (bytes) at `slot`, or 0 if the slot has never been
    /// touched. Mostly useful for tests that want to assert the no-shrink
    /// invariant.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/instance-buffer-pool-slice-4d.md#interface
    /// @issue #1722
    pub fn capacity(&self, slot: usize) -> wgpu::BufferAddress {
        self.slots
            .get(slot)
            .and_then(|s| s.as_ref())
            .map(|s| s.capacity)
            .unwrap_or(0)
    }

    /// Reuse-or-grow the buffer at `slot` so it can hold at least
    /// `min_size_bytes` of payload, then upload `data` into it via
    /// `Queue::write_buffer`. Returns a reference to the underlying
    /// `wgpu::Buffer` for binding.
    ///
    /// Growth policy: if `capacity < min_size_bytes`, new capacity is
    /// `max(min_size_bytes, capacity * 3 / 2)` rounded up to 16 bytes.
    /// If `capacity >= min_size_bytes`, the existing buffer is reused
    /// without realloc and `data` is just written into it (the no-shrink
    /// invariant).
    ///
    /// # Panics
    ///
    /// - `min_size_bytes == 0` — there is no use case for an empty
    ///   allocation; calling this with zero indicates a caller bug.
    /// - `data.len() > min_size_bytes` — the caller pinky-promises the
    ///   payload fits in the reservation. Mismatch is a caller bug.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/instance-buffer-pool-slice-4d.md#interface
    /// @issue #1722
    pub fn get_or_grow(
        &mut self,
        slot: usize,
        min_size_bytes: wgpu::BufferAddress,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        data: &[u8],
    ) -> &wgpu::Buffer {
        assert!(min_size_bytes > 0, "min_size_bytes must be > 0");
        assert!(
            data.len() as wgpu::BufferAddress <= min_size_bytes,
            "data.len() ({}) exceeds reservation ({})",
            data.len(),
            min_size_bytes,
        );

        if slot >= self.slots.len() {
            self.slots.resize_with(slot + 1, || None);
        }

        let need_grow = match &self.slots[slot] {
            Some(s) => s.capacity < min_size_bytes,
            None => true,
        };

        if need_grow {
            let current = self.slots[slot].as_ref().map(|s| s.capacity).unwrap_or(0);
            let new_capacity = next_capacity(current, min_size_bytes);
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("instance_pool_slot"),
                size: new_capacity,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.slots[slot] = Some(PoolSlot {
                buffer,
                capacity: new_capacity,
            });
        }

        let slot_ref = self.slots[slot].as_ref().expect("slot just populated");
        let data_len = data.len() as wgpu::BufferAddress;
        if data_len > 0 {
            match pick_staging_path(data_len, self.staging_threshold_bytes) {
                StagingPath::Direct => {
                    queue.write_buffer(&slot_ref.buffer, 0, data);
                }
                StagingPath::Staged => {
                    // BufferSize is NonZeroU64; data_len > 0 guarantees Some(_).
                    let size =
                        wgpu::BufferSize::new(data_len).expect("data_len > 0 verified above");
                    match queue.write_buffer_with(&slot_ref.buffer, 0, size) {
                        Some(mut view) => view.copy_from_slice(data),
                        // Queue arena refused the staging allocation
                        // (rare — hits the internal hard cap). Fall
                        // back to write_buffer so the upload still
                        // lands rather than silently dropping.
                        None => queue.write_buffer(&slot_ref.buffer, 0, data),
                    }
                }
            }
        }
        &slot_ref.buffer
    }
}

/// Compute the next buffer capacity given the current capacity and the
/// minimum required size. New capacity = max(min, current * 3 / 2), rounded
/// up to a 16-byte boundary.
///
/// Free-standing so the growth policy can be unit-tested without a GPU.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/instance-buffer-pool-slice-4d.md#scope
/// @issue #1722
fn next_capacity(current: wgpu::BufferAddress, min: wgpu::BufferAddress) -> wgpu::BufferAddress {
    let one_point_five = current.saturating_mul(3) / 2;
    let raw = std::cmp::max(min, one_point_five);
    // Round up to 16 bytes. WebGPU's COPY alignment requirement is 4 bytes;
    // 16 is the next-power-of-two ceiling that comfortably covers it and
    // matches the alignment of common instance structs (e.g. CellInstance's
    // vec4 color).
    (raw + 15) & !15
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_capacity_first_allocation_uses_min() {
        // current=0, min=10 -> round up to 16
        assert_eq!(next_capacity(0, 10), 16);
        // current=0, min=32 -> already 16-aligned, stays 32
        assert_eq!(next_capacity(0, 32), 32);
    }

    #[test]
    fn next_capacity_growth_uses_1_5x_when_larger() {
        // current=64, min=80 -> max(80, 96) = 96
        assert_eq!(next_capacity(64, 80), 96);
        // current=1000, min=1001 -> max(1001, 1500) = 1500 -> round to 1504
        assert_eq!(next_capacity(1000, 1001), 1504);
    }

    #[test]
    fn next_capacity_growth_uses_min_when_demand_exceeds_1_5x() {
        // current=10, min=100 -> max(100, 15) = 100 -> already 16-aligned? 100->112
        assert_eq!(next_capacity(10, 100), 112);
        // current=16, min=1024 -> max(1024, 24) = 1024 (already aligned)
        assert_eq!(next_capacity(16, 1024), 1024);
    }

    #[test]
    fn next_capacity_rounds_up_to_16() {
        assert_eq!(next_capacity(0, 1), 16);
        assert_eq!(next_capacity(0, 15), 16);
        assert_eq!(next_capacity(0, 16), 16);
        assert_eq!(next_capacity(0, 17), 32);
        assert_eq!(next_capacity(0, 33), 48);
    }

    #[test]
    fn pick_staging_path_below_threshold_is_direct() {
        // Tiny payload — `write_buffer` wins.
        assert_eq!(pick_staging_path(1, 1024), StagingPath::Direct);
        assert_eq!(pick_staging_path(1023, 1024), StagingPath::Direct);
    }

    #[test]
    fn pick_staging_path_at_threshold_is_staged() {
        // Boundary is `>=` — exactly threshold opts into the staged
        // path so the contract reads "at or above" consistently.
        assert_eq!(pick_staging_path(1024, 1024), StagingPath::Staged);
    }

    #[test]
    fn pick_staging_path_above_threshold_is_staged() {
        // 2 MiB default — a typical large upload.
        assert_eq!(
            pick_staging_path(
                DEFAULT_STAGING_THRESHOLD_BYTES + 1,
                DEFAULT_STAGING_THRESHOLD_BYTES
            ),
            StagingPath::Staged,
        );
    }

    #[test]
    fn pick_staging_path_empty_payload_is_direct() {
        // Symbolic — caller skips the upload anyway, but the policy
        // must agree on the dead-code path.
        assert_eq!(pick_staging_path(0, 1024), StagingPath::Direct);
        assert_eq!(pick_staging_path(0, 0), StagingPath::Direct);
    }

    #[test]
    fn pick_staging_path_zero_threshold_forces_staged() {
        // Escape hatch — `threshold == 0` means every non-empty upload
        // takes the staged path.
        assert_eq!(pick_staging_path(1, 0), StagingPath::Staged);
        assert_eq!(pick_staging_path(u64::MAX, 0), StagingPath::Staged);
    }

    #[test]
    fn pool_default_staging_threshold_matches_const() {
        let pool = InstanceBufferPool::new();
        assert_eq!(
            pool.staging_threshold_bytes(),
            DEFAULT_STAGING_THRESHOLD_BYTES
        );
        // Sanity-check the const itself — 64K × 32-byte CellInstance.
        assert_eq!(DEFAULT_STAGING_THRESHOLD_BYTES, 64 * 1024 * 32);
    }

    #[test]
    fn pool_with_staging_threshold_overrides_default() {
        let pool = InstanceBufferPool::with_staging_threshold(4096);
        assert_eq!(pool.staging_threshold_bytes(), 4096);
        assert!(pool.is_empty());
    }

    #[test]
    fn pool_set_staging_threshold_bytes_mutates() {
        let mut pool = InstanceBufferPool::new();
        pool.set_staging_threshold_bytes(8192);
        assert_eq!(pool.staging_threshold_bytes(), 8192);
        pool.set_staging_threshold_bytes(0);
        assert_eq!(pool.staging_threshold_bytes(), 0);
    }

    #[test]
    fn pool_starts_empty() {
        let pool = InstanceBufferPool::new();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(0), 0);
        assert_eq!(pool.capacity(42), 0);
    }

    #[test]
    fn default_matches_new() {
        let p1 = InstanceBufferPool::new();
        let p2 = InstanceBufferPool::default();
        assert_eq!(p1.len(), p2.len());
        assert!(p1.is_empty());
        assert!(p2.is_empty());
    }

    /// Live-GPU first-allocation check. Requires a real wgpu adapter; the
    /// `#[ignore]` keeps headless CI green. Run via
    /// `cargo test -p jet-grid-render-webgpu -- --ignored` on a host
    /// with a GPU.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/instance-buffer-pool-slice-4d.md#acceptance-criteria
    #[test]
    #[ignore]
    fn get_or_grow_first_allocation_live() {
        let (device, queue) =
            pollster::block_on(headless_device_queue()).expect("headless adapter unavailable");
        let mut pool = InstanceBufferPool::new();
        let data = [1u8, 2, 3, 4, 5, 6, 7, 8];
        let buffer = pool.get_or_grow(0, 64, &device, &queue, &data);
        assert_eq!(buffer.size(), 64);
        assert_eq!(pool.capacity(0), 64);
        assert_eq!(pool.len(), 1);
    }

    /// Live-GPU growth check. After two `get_or_grow` calls with
    /// increasing demand, capacity must follow the documented policy
    /// (1.5× current, floored at min, rounded to 16).
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/instance-buffer-pool-slice-4d.md#acceptance-criteria
    #[test]
    #[ignore]
    fn get_or_grow_growth_live() {
        let (device, queue) =
            pollster::block_on(headless_device_queue()).expect("headless adapter unavailable");
        let mut pool = InstanceBufferPool::new();
        pool.get_or_grow(0, 64, &device, &queue, &[]);
        assert_eq!(pool.capacity(0), 64);
        pool.get_or_grow(0, 100, &device, &queue, &[]);
        // current=64, min=100 -> max(100, 96) = 100 -> 112
        assert_eq!(pool.capacity(0), 112);
    }

    /// Live-GPU no-shrink check. After a large allocation, requesting a
    /// smaller buffer must reuse the existing one without shrinking.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/instance-buffer-pool-slice-4d.md#acceptance-criteria
    #[test]
    #[ignore]
    fn get_or_grow_no_shrink_live() {
        let (device, queue) =
            pollster::block_on(headless_device_queue()).expect("headless adapter unavailable");
        let mut pool = InstanceBufferPool::new();
        pool.get_or_grow(0, 1024, &device, &queue, &[]);
        let cap_before = pool.capacity(0);
        assert_eq!(cap_before, 1024);
        pool.get_or_grow(0, 16, &device, &queue, &[]);
        assert_eq!(pool.capacity(0), cap_before, "pool must not shrink");
    }

    /// Slice #2191: distinct slots — slot 0 (cell pass) and slot 1
    /// (text pass) — must allocate independent buffers and resize
    /// independently. After growing slot 1 past slot 0's size, slot
    /// 0's capacity is preserved verbatim and the underlying
    /// `wgpu::Buffer` handles are distinct.
    #[test]
    #[ignore]
    fn slot_zero_and_one_grow_independently_live() {
        let (device, queue) =
            pollster::block_on(headless_device_queue()).expect("headless adapter unavailable");
        let mut pool = InstanceBufferPool::new();
        let slot0 = pool.get_or_grow(0, 64, &device, &queue, &[]).clone();
        let slot1 = pool.get_or_grow(1, 256, &device, &queue, &[]).clone();
        assert_eq!(pool.capacity(0), 64);
        assert_eq!(pool.capacity(1), 256);
        // Distinct allocations must report distinct sizes — slot 0
        // sized at 64, slot 1 at 256. If the pool were aliasing a
        // single backing buffer across the two slots, both handles
        // would report the same size.
        assert_ne!(
            slot0.size(),
            slot1.size(),
            "slot 0 and slot 1 must hand out distinct buffers"
        );
        let cap0_before = pool.capacity(0);
        pool.get_or_grow(1, 1024, &device, &queue, &[]);
        assert_eq!(
            pool.capacity(0),
            cap0_before,
            "growing slot 1 must not perturb slot 0"
        );
    }

    #[allow(dead_code)]
    async fn headless_device_queue() -> Option<(wgpu::Device, wgpu::Queue)> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await?;
        adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("instance_pool_test_device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::downlevel_defaults(),
                    ..Default::default()
                },
                None,
            )
            .await
            .ok()
    }
}
