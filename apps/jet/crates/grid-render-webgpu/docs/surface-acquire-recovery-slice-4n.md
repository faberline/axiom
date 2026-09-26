# SurfaceTexture acquire timeout handling — Slice 4n

> Issue: #1732 · Parent epic: #1254 · Slice: 4n

## Problem

`WebGpuRenderer::begin_frame` calls `Surface::get_current_texture()` and
maps every `wgpu::SurfaceError` into a `RenderFrameError` variant, but
it does **not** apply any per-variant recovery. The caller (a React
frame driver, a winit event loop, a benchmark harness) has to know the
wgpu vocabulary by heart and remember:

- `Timeout` is transient — skip this frame, try again next tick.
- `Outdated` means the swap chain is stale w.r.t. the current
  `SurfaceConfiguration` — reconfigure and retry once.
- `Lost` means the surface is gone — full recovery is required before
  any future frame.
- `OutOfMemory` is fatal-recoverable — propagate so the driver can
  decide whether to drop the renderer or surface a UI overlay.

In practice every caller writes the same retry/skip/recover boilerplate
around `begin_frame`. The boilerplate is easy to get wrong: missing the
single-shot retry on `Outdated` causes the frame loop to stall against a
resized window; missing the `Lost` → `try_recover` handoff causes the
renderer to loop on a dead surface forever.

## Scope

In:

- Add a new `WebGpuRenderer::try_acquire_frame` method that wraps
  `begin_frame` and applies the per-variant recovery policy listed
  above. Returns an enum so the caller's frame loop has explicit
  action labels (`Frame` / `Skipped` / `NeedsRecovery`) rather than
  having to re-classify wgpu error variants.
- Extract the policy decision tree into a free `classify_acquire`
  function that maps `Result<(), wgpu::SurfaceError>` → an internal
  `AcquireAction` enum. This is the testable surface — unit tests pin
  each variant's classification without needing a real GPU.
- Log the `Timeout` skip at `tracing::warn!` (transient but
  load-bearing for FPS troubleshooting).
- Keep `begin_frame` unchanged — it remains the raw escape hatch for
  callers that want to apply their own policy (the frame timing
  bench, for example).
- Tests:
  - Pure unit tests on `classify_acquire` covering every
    `wgpu::SurfaceError` variant.
  - Headless GPU-gated test (`#[ignore]` by default) that exercises
    `try_acquire_frame` against a 1×1 surface so the happy path is
    integration-covered when a CI runner has a GPU.

Out:

- Calling `try_recover` automatically when `Lost` is observed. The
  recovery is async; the acquire path is sync. The boundary stays at
  `NeedsRecovery` — the caller composes the async step.
- Retrying more than once on `Outdated`. If the second acquire fails
  for the same reason, escalate (treat as Skipped). Unbounded retry
  on a chronically-stale configuration would hot-loop.
- Touching `RenderFrameError` — its variants already 1:1 with
  `wgpu::SurfaceError` (Slice 4e, #1723); this slice composes them,
  it does not redefine them.

## Interface

```rust
/// Outcome of [`WebGpuRenderer::try_acquire_frame`].
///
/// Variants name the action the caller should take, so the frame loop
/// does not have to re-decode wgpu's error vocabulary.
pub enum AcquireOutcome<'r, 'window> {
    /// Surface texture was acquired (possibly after a single
    /// reconfigure+retry on `Outdated`). Caller should encode the
    /// frame as normal.
    Frame(FrameBuilder<'r, 'window>),

    /// Acquire timed out — caller should skip this frame and try
    /// again on the next tick. Logged at `tracing::warn!` so a
    /// chronically-timing-out surface shows up in operator logs.
    Skipped,

    /// Surface or device is gone. Caller MUST call
    /// [`WebGpuRenderer::try_recover`] before requesting another
    /// frame.
    NeedsRecovery,
}

impl<'window> WebGpuRenderer<'window> {
    /// Acquire the next surface texture and apply the per-variant
    /// recovery policy:
    ///
    /// | wgpu error      | outcome                                      |
    /// |-----------------|----------------------------------------------|
    /// | (success)       | `Frame(builder)`                             |
    /// | `Timeout`       | `Skipped` (logged at `warn!`)                |
    /// | `Outdated`      | reconfigure + retry once, then re-classify   |
    /// | `Lost`          | `NeedsRecovery`                              |
    /// | `OutOfMemory`   | `Err(OutOfMemory)` — propagate to caller     |
    /// | (other)         | `Err(Other(msg))` — propagate to caller      |
    ///
    /// `DeviceLost` (pre-check from `begin_frame`) → `NeedsRecovery`.
    pub fn try_acquire_frame(
        &mut self,
    ) -> Result<AcquireOutcome<'_, 'window>, RenderFrameError>;
}
```

Internal helper (private, the unit-test seam):

```rust
/// Pure classification of a surface-acquire result into an action
/// label. Extracted so the policy is unit-testable without spinning
/// up a GPU.
enum AcquireAction {
    Frame,
    SkipTimeout,
    RetryAfterReconfigure,
    NeedsRecovery,
    Propagate(RenderFrameError),
}

fn classify_acquire(
    result: &Result<(), wgpu::SurfaceError>,
) -> AcquireAction;
```

## Acceptance Criteria

- [x] `WebGpuRenderer::try_acquire_frame` exists and returns
      `AcquireOutcome`.
- [x] `Timeout` outcome is `Skipped` and is logged at `tracing::warn!`.
- [x] `Outdated` outcome reconfigures the surface (no size change) and
      retries `get_current_texture` exactly once before escalating.
- [x] `Lost` outcome maps to `NeedsRecovery`; `DeviceLost` pre-check
      also maps to `NeedsRecovery`.
- [x] `OutOfMemory` propagates as `Err(RenderFrameError::OutOfMemory)`.
- [x] `begin_frame` is unchanged — backward-compatible escape hatch.
- [x] `cargo test -p jet-grid-render-webgpu` passes (including new
      unit tests for `classify_acquire`).
- [x] Module-level doc explains the WHY (caller boilerplate, escalation
      ceiling on `Outdated`).

## Reference Context

- `apps/jet/crates/grid-render-webgpu/src/lib.rs:469-493` — current
  `begin_frame` with `lost_status.is_lost()` pre-check.
- `apps/jet/crates/grid-render-webgpu/src/lib.rs:794-854` —
  `RenderFrameError` definition + `From<wgpu::SurfaceError>`.
- `apps/jet/crates/grid-render-webgpu/docs/render-pass-orchestration-slice-4e.md` —
  parent slice that introduced `RenderFrameError`.
- `apps/jet/crates/grid-render-webgpu/docs/lost-context-recovery-slice-4h.md` —
  defines `try_recover`; this slice routes callers to it via
  `NeedsRecovery`.
- `apps/jet/crates/grid-render-webgpu/src/lib.rs:266-274` — `on_resize`,
  the reconfigure primitive used by the `Outdated` retry path.
