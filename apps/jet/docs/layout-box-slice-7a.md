# Box (CSS box model) — Slice 7a

> Issue: #1798 · Parent epic: #1702 (jet layout) · Slice: 7a

## Problem

Every layout slice that comes after this one (block stack, margin
collapsing, inline reflow, flex containers) needs a single primitive:
the **CSS box model** — content, padding, border, margin. This slice
delivers the smallest, most boring version of that primitive so the
later slices have something to add fields to and compute against.

Three choices pin the shape:

1. **Four nested `Rect`s, not insets.** The classic CSS box model
   represents padding / border / margin as *Edges*: four `Length`
   values per ring (top/right/bottom/left). That's the right shape
   when the formatter is *computing* the box (taking content size +
   declared edges and producing rects). At the rendering / hit-testing
   layer, the consumer wants the **resolved rectangle at each ring**
   directly — `content_rect`, `padding_rect`, `outer_rect` — and
   doesn't care about the per-side breakdown anymore. Storing the
   resolved rects up-front means the formatter pays the four
   additions once at layout time; every downstream consumer (painter,
   hit tester, accessibility tree) reads a single `Rect` field. The
   per-side insets reappear in later slices on demand by subtracting
   neighbouring rects (`padding_rect.x - content_rect.x` = `padding-left`).
2. **`Rect { x, y, width, height: f32 }` in pixel space.** `f32`
   matches the renderer's existing pixel-space coordinate convention
   (e.g. `jet_grid_render_webgpu::shaper::PositionedGlyph` from
   Slice 5j carries `f32` pen positions). Subpixel positions are real
   — a `Box` whose `content_rect.x` is `12.5` is normal in a
   DPR-aware renderer. The painter decides rounding.
3. **`#[repr(C)]` + `offset_of!` tests.** The box layout is a *contract*
   — the WASM bridge to React's reconciler (Slice 10) will eventually
   need to read box fields from a `Memory` view, and any FFI consumer
   keys off field offsets. Pinning the offsets via `offset_of!` tests
   means a refactor that inserts a field or changes alignment breaks
   the test before it breaks the bridge.

Two choices on naming:

- **`Box` shadows `std::boxed::Box`.** The issue's AC names this struct
  `Box`. The shadow is local to the `layout::box_model` module path —
  callers who pull both in write `use std::boxed::Box as StdBox;` (we
  don't need `std::boxed::Box` inside this module so the shadow has
  zero impact here). Renaming to `BoxModel` / `LayoutBox` was tempting
  but trades a one-time prelude name clash for permanent disagreement
  with the spec.
- **No `Default`-on-the-rects shortcut for the constructor.** Manual
  field constructors (`Box::new(content, padding, border, margin)`)
  make the *nesting invariant* explicit — `padding` must contain
  `content`, `border` must contain `padding`, `margin` must contain
  `border`. We accept the four-argument constructor over `Default` so
  the caller cannot accidentally instantiate a degenerate box where
  the rings cross. (Validation of the nesting invariant is out of
  scope for this slice — it's a soft contract documented at the
  constructor; the block formatter slice is the right place to assert
  it.)

## Scope

In:

- New module `jet::layout` with two submodules:
  - `jet::layout::rect` — `pub struct Rect { pub x: f32, pub y: f32, pub width: f32, pub height: f32 }`,
    `#[repr(C)]`, `Copy + Debug + PartialEq + Default`. Pure value
    object — no methods on this slice.
  - `jet::layout::box_model` — `pub struct Box { pub content, padding, border, margin: Rect }`,
    `#[repr(C)]`, `Copy + Debug + PartialEq + Default`. Three
    accessors:
    - `pub fn content_rect(&self) -> Rect` — returns `self.content`.
    - `pub fn padding_rect(&self) -> Rect` — returns `self.padding`.
    - `pub fn outer_rect(&self) -> Rect` — returns `self.margin`
      (the outermost ring; equivalent to the historic
      `margin_rect` / `outer_box_rect`).
- Module-level docs explain the WHY:
  - Why four nested rects instead of per-side insets.
  - Why `f32` pixel space (subpixel positions, painter rounds).
  - Why `#[repr(C)]` + `offset_of!` tests (FFI contract).
  - Why `Box` shadows the prelude.
- Unit tests:
  - `rect_default_is_zero` — `Rect::default()` is `{0, 0, 0, 0}`.
  - `rect_field_offsets_pin_repr_c` — `offset_of!(Rect, x/y/width/height)`
    lands at `0/4/8/12` and `size_of::<Rect>() == 16`.
  - `box_default_is_four_zero_rects` — `Box::default()` is four
    zero rects.
  - `box_field_offsets_pin_repr_c` — `offset_of!(Box, content/padding/border/margin)`
    lands at `0/16/32/48` and `size_of::<Box>() == 64`.
  - `box_accessors_return_corresponding_rects` — pin that
    `content_rect()` / `padding_rect()` / `outer_rect()` return the
    `content` / `padding` / `margin` fields respectively.
  - `box_is_copy_partial_eq` — pin trait derives.

Out:

- `Length { Px(f32), Percent(f32), Auto }` — Slice 7b (#1799).
- `Display { Block | Inline | Flex | Grid | None }` — Slice 7c (#1800).
- Block / inline / flex / grid formatters — Slices 7d–7i.
- Margin-collapsing logic — Slice 7e (#1802).
- Per-side edge accessors (`padding_top`, `border_left`, etc.) —
  these reappear in later slices as derived from the rect deltas.
- Nesting-invariant validation (e.g. `content ⊆ padding ⊆ border ⊆ margin`).
  The block formatter slice is the right place to assert this.
- Hit-testing helpers (`contains_point`, etc.) — paint / event slices.

## Interface

```rust
/// Axis-aligned rectangle in pixel space. `f32` matches the
/// renderer's existing pixel-space convention; subpixel positions
/// are real.
///
/// @spec apps/jet/docs/layout-box-slice-7a.md#interface
/// @issue #1798
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// CSS box model — four nested rectangles, content innermost,
/// margin outermost. See module docs for the WHY behind storing
/// resolved rects (vs per-side insets) and pinning the `#[repr(C)]`
/// layout via `offset_of!` tests.
///
/// @spec apps/jet/docs/layout-box-slice-7a.md#interface
/// @issue #1798
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Box {
    pub content: Rect,
    pub padding: Rect,
    pub border:  Rect,
    pub margin:  Rect,
}

impl Box {
    /// The innermost rectangle — where text and child boxes lay out.
    pub fn content_rect(&self) -> Rect;

    /// The content + padding ring.
    pub fn padding_rect(&self) -> Rect;

    /// The outermost ring — content + padding + border + margin.
    /// Equivalent to the historic CSS "margin rect".
    pub fn outer_rect(&self) -> Rect;
}
```

## Acceptance Criteria

- [x] `Box { content, padding, border, margin: Rect }` — implemented
      as a `#[repr(C)]` value object with pub fields.
- [x] `outer_rect()`, `padding_rect()`, `content_rect()` — three
      accessors, each returns the corresponding field.
- [x] Pin via `offset_of!` tests — `rect_field_offsets_pin_repr_c`
      and `box_field_offsets_pin_repr_c` lock the byte layout (`Rect`
      = 16 B, `Box` = 64 B).
- [x] `cargo test -p jet --lib layout::` passes.
- [x] Module-level docs explain the WHY (nested-rects vs insets,
      `f32` pixel-space, `#[repr(C)]` FFI contract, `Box`-shadows-
      prelude rationale), not just the what.

## Reference Context

- Parent epic [#1702](https://github.com/chrischeng-c4/cclab/issues/1702) —
  jet WebGPU-React renderer layout pipeline.
- Slice 5j (#1759) — `jet_grid_render_webgpu::shaper::PositionedGlyph`
  carries `f32` pen positions in pixel space. The layout's `Rect.x/y`
  flow into the same coordinate system.
- Slice 10 series (#1860 – #1869) — React reconciler bindings.
  The FFI bridge will eventually read `Box` fields from a WASM
  `Memory` view; `#[repr(C)]` + pinned offsets keep that contract
  stable across refactors.
- `std::mem::offset_of!` (Rust 1.77+) — used in tests; no
  third-party dep.
