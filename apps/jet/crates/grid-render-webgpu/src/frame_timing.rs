//! GPU timestamp ring buffer for per-frame timing.
//!
//! Why this module exists: the devtools frame-timing overlay needs to
//! know how long the GPU actually spends executing the cell pass, not
//! how long the CPU spends encoding it. wgpu exposes that via
//! `QuerySet` + `RenderPassTimestampWrites`: write a `BEGIN` timestamp
//! when a pass starts, an `END` timestamp when it ends, resolve into a
//! GPU buffer, copy into a CPU-mappable readback, and `map_async` the
//! readback to read the result.
//!
//! Invariant — optional feature: `Features::TIMESTAMP_QUERY` is not
//! guaranteed on every adapter (Firefox WebGPU today, some older
//! drivers). [`FrameTimingPool::new`] probes the supplied
//! `adapter_features` and silently constructs a permanently-disabled
//! pool when the feature is absent. Every public method on a disabled
//! pool is a no-op that returns `None`/`false` — callers do not need
//! to guard against the feature themselves.
//!
//! Invariant — one-frame map-back latency: timestamps written on
//! frame N are only safe to map *after* frame N's submission has
//! actually completed on the GPU. The pool maintains a small ring of
//! slots (capacity 2 today); frame N writes slot `N % capacity` and
//! kicks off `map_async` on it. The callback writes the resolved
//! duration into a shared `Arc<Mutex<Option<f32>>>` that
//! [`Self::last_frame_gpu_ms`] reads. Polling happens on the next
//! `commit` (non-blocking `Device::poll(Maintain::Poll)`) so the
//! callback from frame N-1 fires by the time frame N's commit returns.

use std::sync::{Arc, Mutex};

/// Number of in-flight timestamp slots. A small burst-capacity ring avoids
/// reusing a readback buffer while interaction-heavy browser frames are still
/// waiting for `map_async` callbacks to flush.
const RING_CAPACITY: usize = 8;

/// Per-frame GPU timestamp ring buffer. Owns the `QuerySet` + resolve
/// + readback buffers for each slot, plus a shared result cell the
/// `map_async` callback updates.
///
/// Disabled (no-op) when the adapter does not expose
/// `Features::TIMESTAMP_QUERY` — see module-level docs.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/gpu-timeline-queries-slice-4i.md#interface
/// @issue #1727
pub struct FrameTimingPool {
    state: PoolState,
    /// Shared result cell. The `map_async` callback writes the
    /// resolved frame duration in milliseconds here; readers
    /// (`last_frame_gpu_ms`) observe it. Wrapped in `Arc` so the
    /// callback (which outlives the borrow stack) can keep its own
    /// reference.
    last_ms: Arc<Mutex<Option<f32>>>,
}

/// Readback map work that must be started only after the frame command
/// buffer has been submitted. WebGPU forbids submitting commands that
/// use a buffer while that buffer is mapped or pending map.
pub(crate) struct PendingFrameTimingReadback {
    readback_buffer: wgpu::Buffer,
    timestamp_period_ns: f32,
    last_ms: Arc<Mutex<Option<f32>>>,
}

impl PendingFrameTimingReadback {
    pub(crate) fn start_mapping(self) {
        let period_ns = self.timestamp_period_ns;
        let last_ms_for_cb = Arc::clone(&self.last_ms);
        let readback_for_cb = self.readback_buffer.clone();
        self.readback_buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                if result.is_err() {
                    return;
                }
                let view = readback_for_cb.slice(..).get_mapped_range();
                // Two u64 ticks: begin, end.
                let mut buf = [0u8; 16];
                buf.copy_from_slice(&view[..16]);
                drop(view);
                readback_for_cb.unmap();
                let begin = u64::from_le_bytes([
                    buf[0], buf[1], buf[2], buf[3], buf[4], buf[5], buf[6], buf[7],
                ]);
                let end = u64::from_le_bytes([
                    buf[8], buf[9], buf[10], buf[11], buf[12], buf[13], buf[14], buf[15],
                ]);
                let delta_ticks = end.saturating_sub(begin) as f32;
                let ms = delta_ticks * period_ns / 1_000_000.0;
                if let Ok(mut cell) = last_ms_for_cb.lock() {
                    *cell = Some(ms);
                }
            });
    }
}

enum PoolState {
    /// Adapter lacked `TIMESTAMP_QUERY` — pool is a permanent no-op.
    Disabled,
    /// Pool is wired against the adapter and has live slots.
    Enabled {
        slots: Vec<Slot>,
        next_slot: usize,
        /// Nanoseconds per timestamp tick. Pulled from
        /// `Queue::get_timestamp_period` at construction.
        timestamp_period_ns: f32,
    },
}

struct Slot {
    query_set: wgpu::QuerySet,
    resolve_buffer: wgpu::Buffer,
    readback_buffer: wgpu::Buffer,
    /// `true` once this slot has been written-to-and-submitted at
    /// least once. The very first frame after start-up should not try
    /// to map a slot that has no submitted work behind it.
    submitted: bool,
}

impl FrameTimingPool {
    /// Build a pool. If `TIMESTAMP_QUERY` is missing from
    /// `adapter_features`, returns a permanently-disabled pool whose
    /// every method is a no-op.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/gpu-timeline-queries-slice-4i.md#interface
    /// @issue #1727
    pub(crate) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        adapter_features: wgpu::Features,
    ) -> Self {
        let last_ms = Arc::new(Mutex::new(None));
        if !adapter_features.contains(wgpu::Features::TIMESTAMP_QUERY) {
            return Self {
                state: PoolState::Disabled,
                last_ms,
            };
        }
        let mut slots = Vec::with_capacity(RING_CAPACITY);
        for i in 0..RING_CAPACITY {
            slots.push(Slot {
                query_set: device.create_query_set(&wgpu::QuerySetDescriptor {
                    label: Some("frame_timing_query_set"),
                    ty: wgpu::QueryType::Timestamp,
                    count: 2,
                }),
                // 2 queries × 8 bytes (u64 ticks each).
                resolve_buffer: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("frame_timing_resolve_buffer"),
                    size: 16,
                    usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                    mapped_at_creation: false,
                }),
                readback_buffer: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("frame_timing_readback_buffer"),
                    size: 16,
                    usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
                submitted: false,
            });
            // Suppress unused-variable warning if RING_CAPACITY is 1.
            let _ = i;
        }
        Self {
            state: PoolState::Enabled {
                slots,
                next_slot: 0,
                timestamp_period_ns: queue.get_timestamp_period(),
            },
            last_ms,
        }
    }

    /// `true` if the pool is wired against a real `TIMESTAMP_QUERY`
    /// adapter; `false` for the no-op fallback.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/gpu-timeline-queries-slice-4i.md#interface
    /// @issue #1727
    pub fn is_enabled(&self) -> bool {
        matches!(self.state, PoolState::Enabled { .. })
    }

    /// Most recently completed frame's GPU duration in milliseconds.
    /// `None` if no frame has completed yet or the pool is disabled.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/gpu-timeline-queries-slice-4i.md#interface
    /// @issue #1727
    pub fn last_frame_gpu_ms(&self) -> Option<f32> {
        self.last_ms.lock().ok().and_then(|g| *g)
    }

    /// Approximate byte footprint of this pool's GPU buffers — `0`
    /// when disabled, else `RING_CAPACITY × (resolve + readback)`.
    /// Surface-independent + tiny, but the
    /// memory report (Slice 4bb / #1746) includes it for completeness.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/gpu-memory-accounting-slice-4bb.md#interface
    /// @issue #1746
    pub(crate) fn buffer_bytes_estimate(&self) -> u64 {
        match &self.state {
            PoolState::Disabled => 0,
            // 16-byte resolve_buffer + 16-byte readback_buffer per slot.
            PoolState::Enabled { slots, .. } => (slots.len() as u64) * 32,
        }
    }

    /// Borrow the active slot's `QuerySet`. Returns `None` when the
    /// pool is disabled — callers in [`crate::frame::FrameBuilder`]
    /// then pass `timestamp_writes: None` on the pass descriptor.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/gpu-timeline-queries-slice-4i.md#interface
    /// @issue #1727
    pub(crate) fn active_query_set(&self) -> Option<&wgpu::QuerySet> {
        match &self.state {
            PoolState::Disabled => None,
            PoolState::Enabled {
                slots, next_slot, ..
            } => Some(&slots[*next_slot].query_set),
        }
    }

    /// Encode the resolve + copy commands for the active slot into
    /// `encoder` and rotate to the next slot. Returns readback map
    /// work that the caller must start after queue submission. No-op
    /// when the pool is disabled.
    ///
    /// Caller MUST invoke this exactly once per frame on the same
    /// encoder that wrote the BEGIN/END timestamps, before
    /// `encoder.finish()`. The map callback writes into the shared
    /// result cell observable via [`Self::last_frame_gpu_ms`].
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/gpu-timeline-queries-slice-4i.md#interface
    /// @issue #1727
    pub(crate) fn finish_frame(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
    ) -> Option<PendingFrameTimingReadback> {
        let PoolState::Enabled {
            slots,
            next_slot,
            timestamp_period_ns,
        } = &mut self.state
        else {
            return None;
        };
        let idx = *next_slot;
        let slot = &mut slots[idx];

        encoder.resolve_query_set(&slot.query_set, 0..2, &slot.resolve_buffer, 0);
        encoder.copy_buffer_to_buffer(&slot.resolve_buffer, 0, &slot.readback_buffer, 0, 16);

        let pending = PendingFrameTimingReadback {
            readback_buffer: slot.readback_buffer.clone(),
            timestamp_period_ns: *timestamp_period_ns,
            last_ms: Arc::clone(&self.last_ms),
        };
        slot.submitted = true;

        *next_slot = (idx + 1) % RING_CAPACITY;
        Some(pending)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The disabled pool is the only path host-target tests can build
    /// without a live GPU. We exercise it via a thin manual constructor
    /// that mirrors what `new` would do when `TIMESTAMP_QUERY` is
    /// absent.
    fn disabled_pool() -> FrameTimingPool {
        FrameTimingPool {
            state: PoolState::Disabled,
            last_ms: Arc::new(Mutex::new(None)),
        }
    }

    #[test]
    fn disabled_pool_reports_not_enabled() {
        let pool = disabled_pool();
        assert!(!pool.is_enabled());
    }

    #[test]
    fn disabled_pool_has_no_frame_time() {
        let pool = disabled_pool();
        assert_eq!(pool.last_frame_gpu_ms(), None);
    }

    #[test]
    fn disabled_pool_active_query_set_is_none() {
        let pool = disabled_pool();
        assert!(pool.active_query_set().is_none());
    }

    /// `finish_frame` on a disabled pool is a no-op — it must not
    /// touch the encoder. We can't build a real encoder here without a
    /// GPU, so this is a compile-only witness that the method takes
    /// `&mut CommandEncoder` and returns optional post-submit map work.
    #[allow(dead_code)]
    fn finish_frame_signature_is_stable() {
        let _: fn(
            &mut FrameTimingPool,
            &mut wgpu::CommandEncoder,
        ) -> Option<PendingFrameTimingReadback> = FrameTimingPool::finish_frame;
    }

    /// Compile-time witness: the public timing accessor returns
    /// `Option<f32>` — that's the contract the devtools overlay
    /// (parent epic) reads against. If the type changes, this test
    /// stops compiling.
    #[allow(dead_code)]
    fn last_frame_gpu_ms_signature_is_stable() {
        let _: fn(&FrameTimingPool) -> Option<f32> = FrameTimingPool::last_frame_gpu_ms;
    }
}
