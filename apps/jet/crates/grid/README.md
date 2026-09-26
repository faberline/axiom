# Cclab Grid

## Brief

Cclab Grid is the unified Rust spreadsheet engine crate for cclab grid
workflows.

It owns the sparse sheet/workbook core, formula parser and evaluator,
undo/redo history, feature-gated database persistence, range/spatial query
helpers, and the feature-gated `jet-grid-server` collaboration service. The
server binary compiles under the default features, but HTTP/WebSocket journey
coverage is still tracked as smoke-level evidence rather than full conformance.

## Capabilities

A promise with no gate under it is not claimed.

### Capability Index

| Capability | Root WI | Notes |
|---|---:|---|
| Spreadsheet Core And Editing State | - | sheet/workbook data model, formatting, validation, search, selection/input/viewport state, and command history |
| Formula Evaluation Engine | - | formula parser/evaluator, functions, cross-sheet references, dependency graph, and reference shifting |
| Grid Persistence And Spatial Query | - | feature-gated Morton/WAL cell storage, query builders, and Yrs snapshot/update store |
| Collaboration Server And Workbook API | - | `jet-grid-server` and server modules compile; HTTP/WebSocket journey coverage is still missing |

### Spreadsheet Core And Editing State

Cclab Grid provides the Rust spreadsheet core for sparse sheets, workbooks,
formatting, validation, search/replace, interactive grid state, and undo/redo
command history.

- Root WI: none; this capability predates the tracker.
- Surface: Rust API:
  `jet_grid::core::{Sheet, Workbook, Cell, CellValue, CellRange, CellFormat, DataValidationRule, SearchEngine, SpreadsheetState}`
  - spreadsheet model and editing-state surface.
- Surface: Rust API: `jet_grid::history::{HistoryManager, Command}` -
  undo/redo command history surface.
- Gate — behavior: `cargo test -p jet-grid` - cell values, chunked grid,
  sheet/workbook operations, formatting, validation, search/replace, selection,
  clipboard, input/edit/viewport state, and history commands
- Gate: `cargo test -p jet-grid`
- Source: `apps/jet/crates/grid/src/core`, `apps/jet/crates/grid/src/history`
- Evidence: `cargo test -p jet-grid`; apps/jet/crates/grid/src/core;
  apps/jet/crates/grid/src/history

### Formula Evaluation Engine

Cclab Grid parses and evaluates spreadsheet formulas, resolves cross-sheet
references, tracks formula dependencies, and shifts references across
row/column mutations.

- Root WI: none; this capability predates the tracker.
- Surfaces: Rust API:
  `jet_grid::formula::{NomParser, Parser, Evaluator, CrossSheetEvaluator, DependencyGraph, evaluate_formula, evaluate_formula_cross_sheet}`
  - formula parsing, evaluation, dependency, and reference-shift surface
- Gate — behavior: `cargo test -p jet-grid` - lexer/parser behavior,
  arithmetic/comparison/string/function evaluation, cross-sheet references,
  lookup/logical/math/date/text functions, dependency cycles, and reference
  shifting
- Gate: `cargo test -p jet-grid`
- Source: `apps/jet/crates/grid/src/formula`
- Evidence: `cargo test -p jet-grid`; apps/jet/crates/grid/src/formula

### Grid Persistence And Spatial Query

Cclab Grid persists spreadsheet cells and collaboration snapshots through a
feature-gated database layer with Morton-keyed cell storage, WAL replay,
range/spatial queries, and Yrs update compaction.

- Root WI: none; this capability predates the tracker.
- Surface: Cargo feature: `db` - enables the grid database module.
- Surface: Rust API:
  `jet_grid::db::{CellStore, MortonKey, StoredCell, RangeQuery, SpatialQuery, YrsStore}`
  - storage/query/snapshot persistence surface.
- Gate — behavior: `cargo test -p jet-grid` - Morton
  encode/decode/order/locality, WAL write/replay, cell-store recovery/range
  query/delete, range/spatial query builders, and Yrs update/snapshot store
  operations
- Gate: `cargo test -p jet-grid`
- Source: `apps/jet/crates/grid/src/db`
- Evidence: `cargo test -p jet-grid`; apps/jet/crates/grid/src/db

### Collaboration Server And Workbook API

Cclab Grid exposes a workbook/collaboration service surface through the
`jet-grid-server` binary, environment-based server config, REST workbook
routes, and a Yrs-backed collaboration WebSocket router.

- Root WI: none; this capability predates the tracker.
- Surface: CLI: `jet-grid-server` - starts the Axum workbook/collaboration
  server from `HOST`, `PORT`, and `DATABASE_PATH`.
- Surface: Cargo feature: `server` - enables Axum, cclab-kv storage, Yrs
  collaboration, REST routes, and WebSocket router.
- Surface: Rust API: `jet_grid::server::{run_server, Config}` - server
  startup/config surface.
- Gate — behavior: `cargo test -p jet-grid` - server modules and binary
  compile under default features
- Gate: smoke coverage only because no HTTP/WebSocket journey test is present
- Gate: `cargo test -p jet-grid`
- Source: `apps/jet/crates/grid/src/bin/jet-grid-server.rs`,
  `apps/jet/crates/grid/src/server`
- Evidence: `cargo test -p jet-grid`;
  apps/jet/crates/grid/src/bin/jet-grid-server.rs; apps/jet/crates/grid/src/server
