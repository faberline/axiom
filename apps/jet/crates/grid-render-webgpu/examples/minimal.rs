//! Idiomatic end-to-end example for `jet-grid-render-webgpu`.
//!
//! Run with:
//!
//! ```text
//! cargo run --example minimal -p jet-grid-render-webgpu
//! ```
//!
//! What this example does: drives a single rendered frame through
//! the surface-less smoke harness in [`jet_grid_render_webgpu::headless`].
//! That harness uses the same cell-rect pipeline the real
//! [`jet_grid_render_webgpu::WebGpuRenderer`] runs, but writes
//! into an offscreen `Rgba8UnormSrgb` texture instead of a swap
//! chain — so no window system is needed. The example exists as
//! the "if you read one file to learn this crate, read this one"
//! entry point.
//!
//! Skip semantics: when no wgpu adapter is reachable (typical CI
//! worker that lacks both a software fallback like `llvmpipe`
//! and any hardware GPU), the example prints a skip message and
//! exits `0`. This mirrors the contract the
//! `tests/headless_smoke.rs` integration test pioneered in
//! Slice 4t (#1738) — an unreachable adapter is *not* a
//! regression.
//!
//! @spec apps/jet/crates/grid-render-webgpu/docs/lifecycle-docs-slice-4dd.md#interface
//! @issue #1748

use jet_grid_render_webgpu::cell_rect::CellInstance;
use jet_grid_render_webgpu::headless::{request_smoke_adapter, HeadlessSmokeRenderer};

fn main() {
    pollster::block_on(run());
}

async fn run() {
    let Some((_instance, adapter)) = request_smoke_adapter().await else {
        // The headless integration test (Slice 4t) treats this as
        // a pass-with-message; the example follows the same
        // contract so `cargo run --example minimal` doesn't fail
        // on adapter-less CI workers.
        println!("minimal example: no wgpu adapter reachable — skipping (this is OK)");
        return;
    };

    let size_px = (64u32, 64u32);
    let mut renderer = match HeadlessSmokeRenderer::new(adapter, size_px).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("minimal example: headless renderer construction failed: {e}");
            std::process::exit(1);
        }
    };

    // One coloured 48x48 rect on a white 64x64 clear — small enough
    // that a human reading the docs can verify the picture in their
    // head without running a frame buffer viewer.
    let cell = CellInstance {
        pos_px: [8.0, 8.0],
        size_px: [48.0, 48.0],
        color: [0.2, 0.6, 0.9, 1.0],
    };
    let white = [1.0_f32; 4];

    let pixels = match renderer.render_single_cell(cell, white).await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("minimal example: render_single_cell failed: {e}");
            std::process::exit(1);
        }
    };

    let (w, h) = renderer.size_px();
    println!(
        "minimal example: rendered one frame: {}x{}, {} bytes",
        w,
        h,
        pixels.len()
    );
    // Sanity: tightly packed Rgba8 should be width × height × 4 bytes.
    debug_assert_eq!(pixels.len(), (w * h * 4) as usize);
}
