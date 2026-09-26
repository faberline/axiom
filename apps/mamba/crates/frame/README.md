# Cclab Frame

## Brief

Cclab Frame is the Rust DataFrame and Series library for cclab data workflows.

It owns pandas-like tabular data structures, typed values, single and
multi-indexing, row/column selection, null handling, analytical operations,
reshape/window helpers, and tabular IO. The public surface is a Rust library
API; this crate does not expose a standalone CLI surface.

## Capabilities

A promise with no gate under it is not claimed.

### Capability Index

| Capability | Root WI | Notes |
|---|---:|---|
| DataFrame Series Core | - | DataFrame/Series/Value/Index APIs with pandas-like indexing, null handling, sorting, apply, and stats |
| Analytical Operations | - | GroupBy, joins, reshape, rolling, expanding, and EWM operations |
| Frame IO And Workbook | - | CSV, columnar, workbook, and optional JSON IO surfaces |

### DataFrame Series Core

Cclab Frame provides pandas-like DataFrame and Series primitives with typed
values, single and multi-indexing, positional/label access, null handling,
row/column transforms, sorting, statistics, and conversion helpers.

- Root WI: none; this capability predates the tracker.
- Surfaces: Rust API:
  `frame::frame::{DataFrame, Series, Value, Index, MultiIndex, FrameError}`
- Gate — behavior: `cargo test -p frame` - construction, shape, indexing,
  selection, sorting, null handling, arithmetic, conversion, statistics,
  duplicate handling, and record/dict transforms
- Gate: `cargo test -p frame`
- Evidence: `cargo test -p frame`

### Analytical Operations

Cclab Frame exposes analytical DataFrame operations for grouped aggregation,
joins, pivoting, reshaping, rolling windows, expanding windows, and
exponentially weighted calculations.

- Root WI: none; this capability predates the tracker.
- Surfaces: Rust API: `frame::frame::ops::{GroupBy, JoinType, AggFunc}`
  plus DataFrame reshape/window methods
- Gate — behavior: `cargo test -p frame` - groupby
  aggregates/transform/filter, joins, pivot/crosstab, stack/unstack, rolling,
  expanding, and EWM behavior
- Gate: `cargo test -p frame`
- Evidence: `cargo test -p frame`

### Frame IO And Workbook

Cclab Frame reads and writes tabular data through CSV, columnar, workbook, and
optional JSON feature surfaces while preserving the DataFrame value model.

- Root WI: none; this capability predates the tracker.
- Surface: Rust API:
  `frame::frame::io::{read_csv, write_csv, read_columnar, write_columnar, Workbook}`
  - CSV, columnar, and workbook IO helpers.
- Surface: Cargo feature: `io-extra` - JSON read/write helpers.
- Gate — behavior: `cargo test -p frame` - CSV parse/read/write, columnar
  file roundtrip, workbook sheet behavior, and IO error paths.
- Gate: `cargo test -p frame`
- Evidence: `cargo test -p frame`
