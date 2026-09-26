//! Device-pixel-ratio scaling helpers.
//!
//! Why this module exists: the WebGPU surface configures in *physical*
//! pixels (the GPU draws at the device's true resolution), but React /
//! CSS operate in *logical* pixels (the size the layout engine knows
//! about). On a Retina display at `dpr = 2`, a `100 × 100` CSS box
//! is `200 × 200` physical pixels. Crisp rendering needs the surface
//! sized to physical; correct hit-testing needs the conversion math
//! to go the other way.
//!
//! Invariant — surface always physical: every code path that touches
//! `wgpu::SurfaceConfiguration::{width, height}` MUST pass physical
//! pixels. Logical-pixel callers go through [`compute_physical_size`]
//! before reaching the surface. CSS handles the visual downscale.
//!
//! Invariant — defensive `dpr` floor: a misconfigured caller passing
//! `dpr <= 0` would either zero-divide (in `physical_to_logical_f32`)
//! or produce a zero-sized surface (which wgpu rejects). The helpers
//! silently clamp `dpr` to `1.0` in those cases — the renderer is
//! still usable, just at logical=physical. This is intentional: the
//! React DPR listener can occasionally emit `0` during teardown / tab
//! suspension, and we'd rather render at 1:1 than panic.

/// Multiply a logical size by `dpr`, round half-away-from-zero, and
/// clamp each axis to ≥ 1 (wgpu rejects zero-sized surface configs).
///
/// `dpr` values `<= 0` are silently clamped to `1.0` — see module
/// docs for the rationale.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/dpr-scaling-slice-4g.md#interface
/// @issue #1725
pub fn compute_physical_size(logical: (u32, u32), dpr: f32) -> (u32, u32) {
    let dpr = if dpr > 0.0 { dpr } else { 1.0 };
    let w = (logical.0 as f32 * dpr).round() as u32;
    let h = (logical.1 as f32 * dpr).round() as u32;
    (w.max(1), h.max(1))
}

/// Multiply f32 logical coordinates by `dpr` to get physical pixels.
/// Used by callers that need sub-pixel precision (e.g. pointer event
/// coordinates) instead of the rounded `compute_physical_size` path.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/dpr-scaling-slice-4g.md#interface
/// @issue #1725
pub fn logical_to_physical_f32(logical: (f32, f32), dpr: f32) -> (f32, f32) {
    let dpr = if dpr > 0.0 { dpr } else { 1.0 };
    (logical.0 * dpr, logical.1 * dpr)
}

/// Divide f32 physical coordinates by `dpr` to get logical pixels.
/// This is the conversion the hit-test path uses: pointer events
/// arrive in physical px (from the canvas), the element bbox tree
/// stores logical px, so dividing here lets the tree query work
/// directly. `dpr <= 0` is treated as `1.0` — see module docs.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/dpr-scaling-slice-4g.md#interface
/// @issue #1725
pub fn physical_to_logical_f32(physical: (f32, f32), dpr: f32) -> (f32, f32) {
    let dpr = if dpr > 0.0 { dpr } else { 1.0 };
    (physical.0 / dpr, physical.1 / dpr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dpr_2_doubles_logical_size() {
        // AC: "Test: 100×100 logical canvas at dpr=2 produces 200×200
        // surface texture".
        assert_eq!(compute_physical_size((100, 100), 2.0), (200, 200));
    }

    #[test]
    fn dpr_1_is_identity() {
        assert_eq!(compute_physical_size((100, 100), 1.0), (100, 100));
    }

    #[test]
    fn fractional_dpr_rounds_half_away_from_zero() {
        // dpr = 1.5 (Windows 125% scale): 100 * 1.5 = 150.
        assert_eq!(compute_physical_size((100, 100), 1.5), (150, 150));
        // dpr = 2.625 (Pixel): 80 * 2.625 = 210.
        assert_eq!(compute_physical_size((80, 80), 2.625), (210, 210));
    }

    #[test]
    fn zero_logical_clamps_to_one() {
        // wgpu rejects zero-sized surface configs — the renderer
        // clamps via this helper.
        assert_eq!(compute_physical_size((0, 0), 2.0), (1, 1));
        assert_eq!(compute_physical_size((100, 0), 2.0), (200, 1));
    }

    #[test]
    fn non_positive_dpr_falls_back_to_one() {
        // dpr <= 0 is a misconfiguration; clamp to 1.0 so the
        // renderer stays usable rather than panicking.
        assert_eq!(compute_physical_size((100, 100), 0.0), (100, 100));
        assert_eq!(compute_physical_size((100, 100), -1.5), (100, 100));
    }

    #[test]
    fn f32_roundtrip_preserves_value_at_integer_dpr() {
        let physical = logical_to_physical_f32((10.0, 20.0), 2.0);
        assert_eq!(physical, (20.0, 40.0));
        let logical = physical_to_logical_f32(physical, 2.0);
        assert_eq!(logical, (10.0, 20.0));
    }

    #[test]
    fn physical_to_logical_handles_non_positive_dpr() {
        // No NaN / inf — divide-by-zero turned into divide-by-one.
        let logical = physical_to_logical_f32((50.0, 50.0), 0.0);
        assert_eq!(logical, (50.0, 50.0));
        let logical = physical_to_logical_f32((50.0, 50.0), -2.0);
        assert_eq!(logical, (50.0, 50.0));
    }
}
