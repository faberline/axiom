# Adapter feature negotiation + fallback — Slice 4q

> Issue: #1735 · Parent epic: #1254 · Slice: 4q

## Problem

The renderer currently negotiates exactly one optional adapter feature
inline in `WebGpuRenderer::new`: `TIMESTAMP_QUERY` (Slice 4i, #1727).
Two problems compound:

1. The negotiation logic is buried inside the constructor and the
   recovery path (`try_recover`), so adding another optional feature
   means editing both call sites — easy to forget, easy to drift.
2. There's no way for callers (devtools overlays, telemetry, the
   render loop's metric reporter) to ask "did the adapter accept
   timestamps?" without re-deriving it from `Device::features()`.
   `FrameTimingPool::is_enabled` answers that question for one
   feature; we need a structured shape that scales.

The parent epic (#1254) lists at least two more optionals worth
opting into when present: `TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES`
(unblocks BC/ETC/ASTC compressed-texture paths on adapters that
expose them) and (later) `POLYGON_MODE_LINE` (debug wireframe). This
slice introduces the structure that lets the next optional feature
land as a one-line addition rather than a two-site edit.

## Scope

In:

- New `NegotiatedFeatures` struct with `bool` flags per optional
  feature this slice negotiates: `timestamp_query`,
  `texture_adapter_specific_format_features`. (Not an enum — the
  caller-facing question is "is feature X on?", which is exactly a
  per-feature `bool`.)
- New private `negotiate_features(adapter: wgpu::Features) -> (wgpu::Features, NegotiatedFeatures)`
  that returns the requested-feature mask + the per-flag outcome. The
  required-features mask is **empty** unconditionally — the renderer
  must run on every WebGPU-conformant adapter, so we never *require*
  a feature, we just *opt in* when available.
- New `negotiated: NegotiatedFeatures` field on `WebGpuRenderer`,
  populated in `new()` and refreshed in `try_recover()` (same adapter,
  so same outcome — but defensive against a future adapter swap).
- New `WebGpuRenderer::negotiated_features() -> &NegotiatedFeatures`
  getter for callers.
- Unit tests on `negotiate_features` covering: empty adapter, only
  timestamps, only texture-adapter-specific, both. No GPU required.

Out:

- Opting into `POLYGON_MODE_LINE`. The parent epic lists it as
  nice-to-have for debug wireframe, but no current slice consumes it.
  Adding the request without a consumer is dead weight — the next
  debug-overlay slice can extend `NegotiatedFeatures` then.
- Changing the `FrameTimingPool::new` signature. It already takes
  `adapter_features` and degrades to no-op without `TIMESTAMP_QUERY`
  (Slice 4i contract). This slice surfaces the same bit on
  `NegotiatedFeatures` so callers don't have to query the frame
  timing pool to find out.
- Telemetry. Logging the negotiation outcome at construction is a
  separate concern; this slice gives the data, a future slice can
  decide where to ship it.

## Interface

```rust
/// Outcome of negotiating optional adapter features at renderer
/// construction. Each field is `true` iff the adapter exposed the
/// feature AND the renderer requested it (today, request-when-
/// available is unconditional for every feature listed here).
///
/// Callers use this struct as the single source of truth for "is
/// optional feature X available?" — checking `Device::features()`
/// directly is also valid but mixes required and optional bits.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NegotiatedFeatures {
    pub timestamp_query: bool,
    pub texture_adapter_specific_format_features: bool,
}

impl<'window> WebGpuRenderer<'window> {
    /// Borrow the per-feature negotiation outcome captured at
    /// construction (and refreshed by [`Self::try_recover`]).
    pub fn negotiated_features(&self) -> &NegotiatedFeatures;
}
```

Internal helper (unit-test seam):

```rust
/// Map adapter-advertised features to a `(requested, outcome)` pair.
/// The returned `wgpu::Features` mask is what the renderer should
/// pass to `request_device`; the `NegotiatedFeatures` mirrors the
/// successful opt-ins. Pure, no GPU required.
fn negotiate_features(
    adapter: wgpu::Features,
) -> (wgpu::Features, NegotiatedFeatures);
```

## Acceptance Criteria

- [x] Required features: empty — renderer runs on every WebGPU
      adapter.
- [x] Optional features: `TIMESTAMP_QUERY`,
      `TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES` (request when the
      adapter exposes them).
- [x] Per-feature flag on `WebGpuRenderer.negotiated_features()`
      reflects negotiation outcome.
- [x] Frame timing (Slice 4i) only enabled if `TIMESTAMP_QUERY`
      available — already true; this slice surfaces the bit on
      `NegotiatedFeatures` without changing the timing-pool contract.
- [x] `cargo test -p jet-grid-render-webgpu` passes.
- [x] Module-level docs explain the WHY (request-don't-require for
      optionals; struct beats grep over `wgpu::Features` bits).

## Reference Context

- `apps/jet/crates/grid-render-webgpu/src/lib.rs:158-167` — current
  inline negotiation in `new()` (this slice extracts it).
- `apps/jet/crates/grid-render-webgpu/src/lib.rs:870-880` —
  `try_recover` re-uses `self.required_features`; this slice
  refreshes `negotiated` from the adapter too.
- `apps/jet/crates/grid-render-webgpu/src/frame_timing.rs` — Slice 4i
  pool that degrades without `TIMESTAMP_QUERY`.
- `apps/jet/crates/grid-render-webgpu/docs/gpu-timeline-queries-slice-4i.md` —
  Slice 4i contract for the timing pool's no-op degradation.
