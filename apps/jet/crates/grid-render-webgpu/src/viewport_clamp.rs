//! Pixel-coord viewport clamping + cell-rect filtering. Slice 4v
//! (#1740).
//!
//! Why this module exists: raw scroll input from the JS side is
//! oblivious to the sheet's content extent. If we mirror it straight
//! into the viewport uniform's `scroll_px` field (Slice 4s), the user
//! can scroll past the end of the sheet and end up looking at empty
//! space below the last row. Worse, even when the viewport is small
//! and the sheet is large, every frame uploads every cell — the
//! rasterizer rejects the out-of-rect triangles but the upload
//! bandwidth and vertex-shader work are already spent.
//!
//! This module exposes the two pure pixel-coord primitives that close
//! both gaps:
//!
//! - [`clamp_scroll_px`] pins raw scroll to
//!   `[0, max(0, content_extent - viewport_size)]` per axis.
//! - [`visible_rect_px`] + [`cell_intersects_rect`] let a caller drop
//!   cells whose AABB doesn't overlap the visible window before they
//!   hit the instance-buffer upload.
//!
//! Both are free functions so they can be reasoned about (and unit-
//! tested) without standing up a `wgpu::Device`. The renderer wires
//! them in via `WebGpuRenderer::set_scroll` and
//! `WebGpuRenderer::render_frame_clipped`.

use crate::cell_rect::CellInstance;

/// Pin raw scroll to `[0, max(0, content_extent - viewport_size)]`
/// per axis. `content_extent_px = INFINITY` (the renderer's "I don't
/// know how big the sheet is" sentinel) degenerates to
/// `max(0, raw_px)` — floor only.
///
/// NaN-safe: any NaN coordinate clamps to `0.0` (the floor branch
/// short-circuits on the `< 0.0` comparison, which is `false` for
/// NaN, so the comparison cascade naturally falls through to the
/// `min(content - viewport)` step; that comparison is also `false`
/// for NaN, so the final result for a NaN input is the original
/// NaN. We coerce it to `0.0` at the end as a final guard.).
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/viewport-clamping-helper-integration-slice-4v.md#interface
/// @issue #1740
pub fn clamp_scroll_px(
    raw_px: [f32; 2],
    content_extent_px: [f32; 2],
    viewport_size_px: [f32; 2],
) -> [f32; 2] {
    let mut out = [0.0_f32; 2];
    for axis in 0..2 {
        let raw = raw_px[axis];
        let content = content_extent_px[axis];
        let viewport = viewport_size_px[axis];
        let upper = (content - viewport).max(0.0);
        let clamped = if raw.is_nan() {
            0.0
        } else if upper.is_infinite() {
            raw.max(0.0)
        } else {
            raw.clamp(0.0, upper)
        };
        out[axis] = clamped;
    }
    out
}

/// AABB of the visible window in virtual-sheet coords:
/// `min = scroll_px`, `max = scroll_px + viewport_size_px`.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/viewport-clamping-helper-integration-slice-4v.md#interface
/// @issue #1740
pub fn visible_rect_px(scroll_px: [f32; 2], viewport_size_px: [f32; 2]) -> ([f32; 2], [f32; 2]) {
    let min = scroll_px;
    let max = [
        scroll_px[0] + viewport_size_px[0],
        scroll_px[1] + viewport_size_px[1],
    ];
    (min, max)
}

/// `true` iff `cell`'s AABB overlaps the rect (inclusive on min,
/// exclusive on max). The boundary convention matches row-major
/// pixel addressing: a cell at `pos_px = (0, 0)` with `size_px =
/// (W, H)` covers the half-open pixel range `[0, W) × [0, H)`.
///
/// A zero-area cell (`size_px = (0, 0)`) is treated as not visible.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/viewport-clamping-helper-integration-slice-4v.md#interface
/// @issue #1740
pub fn cell_intersects_rect(cell: &CellInstance, min_px: [f32; 2], max_px: [f32; 2]) -> bool {
    let cell_min_x = cell.pos_px[0];
    let cell_min_y = cell.pos_px[1];
    let cell_max_x = cell.pos_px[0] + cell.size_px[0];
    let cell_max_y = cell.pos_px[1] + cell.size_px[1];
    // Zero-area cells contribute nothing to the rasterizer.
    if cell_max_x <= cell_min_x || cell_max_y <= cell_min_y {
        return false;
    }
    cell_min_x < max_px[0]
        && cell_max_x > min_px[0]
        && cell_min_y < max_px[1]
        && cell_max_y > min_px[1]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_scroll_with_finite_extent_pins_at_max() {
        // Sheet is 1000×1000 px; viewport is 200×200; raw asked for
        // 5000,5000. Clamp must pin at 800,800.
        let out = clamp_scroll_px([5000.0, 5000.0], [1000.0, 1000.0], [200.0, 200.0]);
        assert_eq!(out, [800.0, 800.0]);
    }

    #[test]
    fn clamp_scroll_with_viewport_larger_than_content_yields_zero() {
        // If the entire sheet fits in the viewport there's nothing
        // to scroll — clamp to (0, 0) regardless of raw input.
        let out = clamp_scroll_px([99.0, -10.0], [50.0, 50.0], [200.0, 200.0]);
        assert_eq!(out, [0.0, 0.0]);
    }

    #[test]
    fn clamp_scroll_floors_negative_raw_input() {
        let out = clamp_scroll_px([-25.0, -1.0], [1000.0, 1000.0], [200.0, 200.0]);
        assert_eq!(out, [0.0, 0.0]);
    }

    #[test]
    fn clamp_scroll_with_infinite_extent_is_floor_only() {
        // The renderer's default extent is INFINITY — clamp must
        // pass through, only flooring at 0.
        let out = clamp_scroll_px(
            [123.5, -50.0],
            [f32::INFINITY, f32::INFINITY],
            [200.0, 200.0],
        );
        assert_eq!(out, [123.5, 0.0]);
    }

    #[test]
    fn clamp_scroll_nan_coerces_to_zero() {
        let out = clamp_scroll_px([f32::NAN, f32::NAN], [1000.0, 1000.0], [200.0, 200.0]);
        assert_eq!(out, [0.0, 0.0]);
    }

    #[test]
    fn visible_rect_is_scroll_to_scroll_plus_size() {
        let (min, max) = visible_rect_px([10.0, 20.0], [200.0, 300.0]);
        assert_eq!(min, [10.0, 20.0]);
        assert_eq!(max, [210.0, 320.0]);
    }

    #[test]
    fn cell_inside_visible_rect_intersects() {
        let cell = CellInstance {
            pos_px: [50.0, 50.0],
            size_px: [10.0, 10.0],
            color: [1.0, 0.0, 0.0, 1.0],
        };
        assert!(cell_intersects_rect(&cell, [0.0, 0.0], [200.0, 200.0]));
    }

    #[test]
    fn cell_entirely_left_of_visible_rect_misses() {
        // Cell at x=[0, 10), rect starts at x=20.
        let cell = CellInstance {
            pos_px: [0.0, 50.0],
            size_px: [10.0, 10.0],
            color: [1.0, 0.0, 0.0, 1.0],
        };
        assert!(!cell_intersects_rect(&cell, [20.0, 0.0], [200.0, 200.0]));
    }

    #[test]
    fn cell_entirely_below_visible_rect_misses() {
        // Cell at y=[300, 310), rect ends at y=200.
        let cell = CellInstance {
            pos_px: [50.0, 300.0],
            size_px: [10.0, 10.0],
            color: [1.0, 0.0, 0.0, 1.0],
        };
        assert!(!cell_intersects_rect(&cell, [0.0, 0.0], [200.0, 200.0]));
    }

    #[test]
    fn cell_straddling_left_edge_intersects() {
        // Cell at x=[15, 25), rect starts at x=20 — overlap is
        // [20, 25), nonempty.
        let cell = CellInstance {
            pos_px: [15.0, 50.0],
            size_px: [10.0, 10.0],
            color: [1.0, 0.0, 0.0, 1.0],
        };
        assert!(cell_intersects_rect(&cell, [20.0, 0.0], [200.0, 200.0]));
    }

    #[test]
    fn cell_touching_max_edge_from_outside_misses() {
        // Cell at x=[200, 210); rect's max-x is 200 (exclusive).
        // No overlap.
        let cell = CellInstance {
            pos_px: [200.0, 50.0],
            size_px: [10.0, 10.0],
            color: [1.0, 0.0, 0.0, 1.0],
        };
        assert!(!cell_intersects_rect(&cell, [0.0, 0.0], [200.0, 200.0]));
    }

    #[test]
    fn zero_area_cell_never_intersects() {
        let cell = CellInstance {
            pos_px: [50.0, 50.0],
            size_px: [0.0, 0.0],
            color: [1.0, 0.0, 0.0, 1.0],
        };
        assert!(!cell_intersects_rect(&cell, [0.0, 0.0], [200.0, 200.0]));
    }

    #[test]
    fn filter_drops_outside_visible_rect_round_trip() {
        // 100×100 viewport at scroll=(0,0). Build a 4-cell scene:
        // - one fully inside
        // - one fully outside (right)
        // - one fully outside (below)
        // - one straddling the bottom edge
        // Filter should keep exactly the inside cell and the
        // straddler.
        let visible = ([0.0, 0.0], [100.0, 100.0]);
        let cells = [
            CellInstance {
                pos_px: [10.0, 10.0],
                size_px: [10.0, 10.0],
                color: [1.0; 4],
            },
            CellInstance {
                pos_px: [150.0, 10.0],
                size_px: [10.0, 10.0],
                color: [1.0; 4],
            },
            CellInstance {
                pos_px: [10.0, 150.0],
                size_px: [10.0, 10.0],
                color: [1.0; 4],
            },
            CellInstance {
                pos_px: [10.0, 95.0],
                size_px: [10.0, 10.0],
                color: [1.0; 4],
            },
        ];
        let kept: Vec<&CellInstance> = cells
            .iter()
            .filter(|c| cell_intersects_rect(c, visible.0, visible.1))
            .collect();
        assert_eq!(kept.len(), 2);
        assert_eq!(kept[0].pos_px, [10.0, 10.0]);
        assert_eq!(kept[1].pos_px, [10.0, 95.0]);
    }
}
