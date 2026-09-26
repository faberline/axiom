//! End-to-end smoke test for the cell-rect render path against a
//! software-preferred wgpu adapter. Slice 4t (#1738).
//!
//! Why this test exists: the unit tests in this crate cover pure
//! data-plane invariants (struct layout, growth policy, path
//! selection). The actual GPU path — shader compile, viewport bind
//! group, vertex draw, color write — was previously gated behind
//! `#[ignore]` live-GPU markers. This test routes the same path
//! through wgpu's software fallback adapter (Vulkan llvmpipe on Linux
//! CI; Metal/Vulkan on developer hardware) and asserts a single
//! rendered pixel matches its requested color.
//!
//! Skip semantics: if `request_smoke_adapter` returns `None`, no
//! adapter at all is reachable on this host. The test prints a
//! skip message and passes — failing here would just create noise
//! on truly headless hosts (no GPU, no llvmpipe, e.g. some minimal
//! containers).

use jet_grid_render_webgpu::cell_rect::CellInstance;
use jet_grid_render_webgpu::headless::{request_smoke_adapter, HeadlessSmokeRenderer};

#[test]
fn headless_smoke_renders_single_cell() {
    let result = pollster::block_on(async {
        let Some((_instance, adapter)) = request_smoke_adapter().await else {
            return None;
        };
        let mut renderer = HeadlessSmokeRenderer::new(adapter, (8, 8))
            .await
            .expect("HeadlessSmokeRenderer::new failed against an available adapter");
        // Fill the entire 8×8 surface with a distinctive color so a
        // stuck-at-clear (white) default would visibly fail this.
        let cell = CellInstance {
            pos_px: [0.0, 0.0],
            size_px: [8.0, 8.0],
            color: [1.0, 0.25, 0.5, 1.0],
        };
        let pixels = renderer
            .render_single_cell(cell, [1.0, 1.0, 1.0, 1.0])
            .await
            .expect("render_single_cell failed");
        Some(pixels)
    });

    let Some(pixels) = result else {
        eprintln!("[headless_smoke] no wgpu adapter available; skipping (host has no GPU and no software fallback)");
        return;
    };

    assert_eq!(pixels.len(), 8 * 8 * 4, "expected 8x8 RGBA8 row-packed");

    // Center pixel (4, 4). Expect the cell's fill color, sRGB-encoded.
    // The target is Rgba8UnormSrgb so the GPU performs linear→sRGB
    // on write; we compare against the sRGB byte of each linear
    // component within a small epsilon to absorb driver rounding.
    let row_stride = 8 * 4;
    let cx = 4usize;
    let cy = 4usize;
    let i = cy * row_stride + cx * 4;
    let r = pixels[i];
    let g = pixels[i + 1];
    let b = pixels[i + 2];
    let a = pixels[i + 3];

    let expected_r = linear_to_srgb_u8(1.0);
    let expected_g = linear_to_srgb_u8(0.25);
    let expected_b = linear_to_srgb_u8(0.5);
    let eps: i32 = 4; // sRGB encoding + driver rounding tolerance.

    let within =
        |actual: u8, expected: u8| -> bool { (actual as i32 - expected as i32).abs() <= eps };

    assert!(
        within(r, expected_r),
        "red channel: got {r}, expected ≈{expected_r}",
    );
    assert!(
        within(g, expected_g),
        "green channel: got {g}, expected ≈{expected_g}",
    );
    assert!(
        within(b, expected_b),
        "blue channel: got {b}, expected ≈{expected_b}",
    );
    assert!(within(a, 255), "alpha channel: got {a}, expected ≈255",);
}

/// Convert a linear-space `[0, 1]` component into the sRGB-encoded
/// byte the GPU writes into an `Rgba8UnormSrgb` target. The formula
/// matches WGSL's standard sRGB OETF.
fn linear_to_srgb_u8(linear: f32) -> u8 {
    let c = linear.clamp(0.0, 1.0);
    let srgb = if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (srgb * 255.0).round().clamp(0.0, 255.0) as u8
}
