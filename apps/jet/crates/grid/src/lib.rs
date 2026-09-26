// @spec .aw/changes/grid-consolidate/groups/consolidate-grid-crates/specs/grid-crate-structure.md#R1
//! # jet-grid
//!
//! Unified spreadsheet engine crate combining core data structures, formula evaluation,
//! undo/redo history, database persistence, and collaboration server.
//!
//! ## Modules
//!
//! - `core` - Sparse matrix storage, cell types, coordinate system, range operations, sheet management
//! - `formula` - Formula parser, evaluator, function library, dependency graph
//! - `history` - Command-based undo/redo history
//! - `db` - Morton encoding persistence, WAL-backed storage, range queries (feature: `db`)
//! - `server` - Axum web server, CRDT collaboration, WebSocket handlers (feature: `server`)

pub mod core;
pub mod formula;
pub mod history;

#[cfg(feature = "db")]
pub mod db;

#[cfg(feature = "server")]
pub mod server;
