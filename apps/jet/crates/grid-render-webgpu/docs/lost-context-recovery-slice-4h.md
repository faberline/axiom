# Lost-context recovery — Slice 4h

> **Issue**: #1726
> **Parent epic**: #1254 (WebGPU-React renderer)
> **Depends on**: #1719–#1723 (Slices 4a–4e — renderer + `render_frame`),
> #1728 (Slice 4j — `FrameBuilder` / `begin_frame`).
> **Status**: in-flight

## Problem

The GPU device can disappear out from under us at any time:

- A browser tab goes to background long enough that the UA reclaims the
  WebGPU device.
- The platform GPU driver crashes (Windows TDR, Mac kernel reset).
- The adapter is hot-unplugged (eGPU, hybrid laptops switching graphs).

When this happens the existing `wgpu::Device` is permanently dead and
every subsequent `get_current_texture`, `submit`, or pipeline build
will fail. The renderer must:

1. **Observe the loss** the moment it happens, not just on the next
   `render_frame` (so the React layer can put up a "GPU restarting…"
   overlay before the user sees a black frame).
2. **Tear down** the per-device GPU state (pipelines, buffers, bind
   groups, the device itself).
3. **Attempt recovery** by asking the same `Adapter` for a fresh
   `(Device, Queue)`, reconfiguring the persistent `Surface` against
   the new device, and rebuilding the per-device state.
4. **Surface a failure event** to the React layer when recovery itself
   fails (driver permanently gone), so a fallback UI can be shown.

The recovery path piggybacks on existing renderer state — the
`Surface` survives device replacement, and the cell-pass shader text /
buffer descriptors are deterministic, so rebuild is mechanical.

## Scope

In:

- New module `lost_context` with:
  - `DeviceLostEvent { reason, message }` — observable struct that
    pairs wgpu's `DeviceLostReason` with the human-readable string the
    driver supplied.
  - `LostContextStatus` — `Arc`-wrapped shared cell that the
    `Device::set_device_lost_callback` writes into; the renderer
    reads from it.
- `WebGpuRenderer` retains the `Adapter` post-construction so it can
  call `Adapter::request_device` again on recovery.
- `WebGpuRenderer::is_device_lost() -> bool` — non-destructive probe.
- `WebGpuRenderer::take_device_lost_event() -> Option<DeviceLostEvent>` —
  pulls the current event out of the status cell (single-take, so the
  React layer only handles the same loss once).
- `WebGpuRenderer::try_recover() -> Result<(), RecoveryError>` —
  async, drops per-device state, requests a new device, reconfigures
  the surface, rebuilds shader / layouts / viewport / instance pool /
  timing pool. On success the renderer is ready to accept frames again
  and `is_device_lost()` returns `false`.
- New `RenderFrameError::DeviceLost { reason, message }` — emitted by
  `begin_frame` when the loss flag is set, so callers can branch to
  `try_recover` without first hitting a wgpu panic.
- New `RecoveryError` enum with `RequestDevice(String)` and
  `Surface(String)` variants so failure modes are distinguishable.

Out:

- Re-creating the [`Surface`] target. The `SurfaceTarget` (the underlying
  window / canvas handle) is owned by the caller and survives device
  loss in all environments we target. If the surface itself is gone
  (window closed) the React layer will already have torn down the
  renderer; recovering from that is the caller's job.
- Automatic re-render of in-flight frame state (e.g. last-rendered cell
  list). Slice 4h delivers the recovery primitive; replaying the last
  frame is the caller's call.
- Recovery retry policy (backoff, max attempts). The React layer can
  call `try_recover` on its own schedule.
- An actual lost-context smoke test against a real GPU. The wgpu API
  has no portable "force device lost" hook; integration tests rely on
  the `Destroyed` path (calling `Device::destroy` and then observing
  the callback fires) which still needs a live adapter.

## Interface

```rust
// apps/jet/crates/grid-render-webgpu/src/lost_context.rs

/// Snapshot of a single device-lost notification. Built inside the
/// `Device::set_device_lost_callback` and observed via the renderer's
/// `take_device_lost_event` accessor.
///
/// @issue #1726
#[derive(Debug, Clone)]
pub struct DeviceLostEvent {
    pub reason: wgpu::DeviceLostReason,
    pub message: String,
}

impl<'window> WebGpuRenderer<'window> {
    /// `true` once a device-lost callback has fired. Stays `true` until
    /// a successful `try_recover` rebuilds the device.
    pub fn is_device_lost(&self) -> bool;

    /// Single-take accessor for the most recent lost event. Returns
    /// `None` if no loss is pending OR if a previous call already
    /// drained it. The renderer keeps `is_device_lost() == true` even
    /// after the event is taken.
    pub fn take_device_lost_event(&self) -> Option<DeviceLostEvent>;

    /// Drop per-device state, request a fresh `(Device, Queue)` from
    /// the cached `Adapter`, reconfigure the surface, and rebuild
    /// shader / layouts / viewport / instance pool / timing pool.
    /// On success `is_device_lost()` returns `false`. On failure the
    /// renderer stays in the lost state and the error names which step
    /// failed.
    pub async fn try_recover(&mut self) -> Result<(), RecoveryError>;
}

/// Failure modes for `WebGpuRenderer::try_recover`.
#[derive(Debug, thiserror::Error)]
pub enum RecoveryError {
    #[error("adapter rejected request_device: {0}")]
    RequestDevice(String),
    #[error("surface reconfigure failed: {0}")]
    Surface(String),
}

// Added to existing RenderFrameError enum:
#[error("device lost: {reason:?} — {message}")]
DeviceLost { reason: wgpu::DeviceLostReason, message: String },
```

## Acceptance criteria

- [x] Listen for `Device::lost` event — wired in
      `WebGpuRenderer::new` via `device.set_device_lost_callback`
      writing into the shared `LostContextStatus`.
- [x] On lost: tear down all GPU resources — `try_recover` drops
      `cell_pipelines` (HashMap), `cell_shader`, `cell_bind_group_layout`,
      `cell_pipeline_layout`, `viewport_buffer`, `viewport_bind_group`,
      `instance_pool`, `frame_timing`, `device`, `queue` before
      rebuilding.
- [x] Attempt re-init via `Adapter::request_device` — the renderer
      retains `adapter: wgpu::Adapter` post-construction and re-requests
      a fresh device with the same `required_features` mask.
- [x] On re-init success: re-create pipelines, bind groups, atlas,
      etc. — pipeline cache is rebuilt lazily (`HashMap::new`); shader /
      layouts / viewport bind group / instance pool / timing pool are
      built up front the same way `new()` does.
- [x] On failure: emit error event for React layer — surfaced via the
      returned `RecoveryError` from `try_recover`; the originating
      `DeviceLostEvent` remains available via `take_device_lost_event`
      so the React layer can show context-specific copy.
- [x] `cargo test -p jet-grid-render-webgpu` passes.
- [x] Module-level docs explain WHY (browser-revoked-device invariant,
      surface survives device swap, single-take event semantic).

## Reference context

- `apps/jet/crates/grid-render-webgpu/src/lib.rs` — `WebGpuRenderer::new`
  (where the device-lost callback is wired) and `begin_frame` (where
  the loss flag becomes `RenderFrameError::DeviceLost`).
- `apps/jet/crates/grid-render-webgpu/src/frame_timing.rs` — Slice 4i
  pool that's rebuilt on recovery.
- wgpu 24 API:
  - `Device::set_device_lost_callback(impl Fn(DeviceLostReason, String)
    + Send + 'static)` — required `Send` bound + `'static` lifetime
    drives the `Arc<LostContextStatus>` design.
  - `Adapter::request_device(&self, ...)` takes `&self` so the adapter
    handle is reusable across recovery cycles.
  - `wgpu::DeviceLostReason::{Unknown, Destroyed}` — the only two
    variants today; we surface both verbatim so the React layer can
    branch on them.
