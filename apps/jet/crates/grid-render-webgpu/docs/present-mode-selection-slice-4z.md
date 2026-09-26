# Present-mode selection (Fifo / Mailbox) — Slice 4z

> Issue: #1744 · Parent epic: #1254 · Slice: 4z

## Problem

`wgpu::PresentMode` trades latency for power:

- **Mailbox** — the swap chain drops the previous unpresented frame
  if a newer one arrives. Lowest latency at the cost of wasted GPU
  work. Right for native interactive UIs where input-to-photon
  latency dominates UX.
- **Fifo** — vsync-locked, never drops frames. Power-friendly and
  the only viable mode on web (the browser composites with vsync
  anyway, so any other mode is silently translated to Fifo). The
  default for battery-sensitive native sessions.
- **FifoRelaxed** — Fifo but allows tearing when the GPU is below
  refresh. Not currently selected by the policy in this slice; kept
  as documentation in case a downstream tunable wants it.

The renderer was hard-coded to `wgpu::PresentMode::AutoVsync` at
construction (which wgpu translates to Fifo). That's correct for web
but leaves native interactive sessions paying the vsync latency cost
unconditionally. This slice adds (1) a policy fn that picks the right
mode given a build target + power profile + the adapter's *actually
supported* mode list, and (2) a public setter so callers can switch
modes after construction (e.g. when the user toggles "low-power" in
settings).

## Scope

In:

- New `pub enum PresentTarget` with three variants:
  - `Web` — always picks `Fifo`.
  - `NativeInteractive` — prefers `Mailbox`, falls back to `Fifo` if
    the adapter doesn't list Mailbox.
  - `NativeLowPower` — always picks `Fifo`.
- New `pub fn recommend_present_mode(target, supported) -> wgpu::PresentMode`
  — pure function, deterministic, no `wgpu::Device` required. Takes
  the adapter's `surface_caps.present_modes` slice + the target and
  returns the chosen mode. Falls back to `Fifo` if no mode in the
  preferred list is supported (Fifo is required by the WebGPU spec
  on every adapter).
- New `WebGpuRenderer::set_present_mode(mode)` — overwrites
  `surface_config.present_mode` and re-configures the surface. The
  caller is responsible for picking a supported mode (use the policy
  fn for that).
- New `WebGpuRenderer::present_mode() -> wgpu::PresentMode` getter
  for assertions + tests.
- New `WebGpuRenderer::supported_present_modes() -> Vec<wgpu::PresentMode>`
  — snapshot of the adapter's supported list, captured at
  construction. Lets a caller drive `recommend_present_mode` without
  re-querying surface caps.
- Module-level doc on the policy function explaining the
  three-target rubric.
- Unit tests for the policy fn (all 3 targets, with and without
  Mailbox in the supported list).

Out:

- Auto-detecting "low power" mode at runtime (battery API on web,
  thermal pressure on native). The caller (browser layer / OS shell)
  knows; the renderer is the consumer.
- `FifoRelaxed`. The AC names Mailbox/Fifo only; FifoRelaxed is
  documented in `PresentTarget`'s rustdoc as a future tuning knob
  but not selected by the policy in this slice.
- Live integration test that actually reconfigures the surface. The
  surface plumbing is exercised by every existing live test that
  goes through `on_resize`. The setter just sets a field + calls
  `surface.configure` — same call path as resize.

## Interface

```rust
/// Target the renderer is running against. Drives present-mode
/// policy: web is vsync-only; native splits by power profile.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/present-mode-selection-slice-4z.md#interface
/// @issue #1744
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentTarget {
    /// Browser / wasm target. Always `Fifo` — browsers translate
    /// every other mode to it anyway.
    Web,
    /// Native window, user is actively interacting (typing, scrolling,
    /// dragging). Mailbox preferred for lowest latency; Fifo fallback
    /// if the adapter doesn't list Mailbox.
    NativeInteractive,
    /// Native window, the host signaled low-power mode (laptop on
    /// battery, "battery saver" toggle on). Always `Fifo`.
    NativeLowPower,
}

/// Pick a `PresentMode` for `target` from the adapter's `supported`
/// list. Always returns one of the modes in `supported`; falls back
/// to `Fifo` (required by the WebGPU spec on every adapter) if the
/// preferred mode isn't listed.
pub fn recommend_present_mode(
    target: PresentTarget,
    supported: &[wgpu::PresentMode],
) -> wgpu::PresentMode;

impl<'window> WebGpuRenderer<'window> {
    pub fn present_mode(&self) -> wgpu::PresentMode;
    pub fn supported_present_modes(&self) -> &[wgpu::PresentMode];
    pub fn set_present_mode(&mut self, mode: wgpu::PresentMode);
}
```

## Acceptance Criteria

- [x] Web target → `Fifo` — `recommend_present_mode(Web, ..)` is the
      identity-on-Fifo branch.
- [x] Native interactive → Mailbox preferred, Fifo fallback — covered
      by `recommend_native_interactive_prefers_mailbox` +
      `recommend_native_interactive_falls_back_to_fifo` tests.
- [x] Native low-power → Fifo — `recommend_native_low_power_is_fifo`.
- [x] `WebGpuRenderer::set_present_mode(mode)` — overwrites the
      surface config field and re-configures the surface.
- [x] `cargo test -p jet-grid-render-webgpu` passes.
- [x] Module-level doc explains the WHY: present-mode is a
      target-aware policy decision, not a single constant.

## Reference Context

- `apps/jet/crates/grid-render-webgpu/src/lib.rs:227` — current
  `surface_config` construction; `present_mode` was hard-coded to
  `AutoVsync`. Slice 4z parameterizes it.
- `apps/jet/crates/grid-render-webgpu/src/lib.rs` — `on_resize` is the
  existing path that calls `surface.configure`; `set_present_mode`
  mirrors it.
- Parent epic #1254 — WebGPU-React renderer; Slice 4z closes the
  present-mode gap for native interactive UIs.
