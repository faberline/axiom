//! Snapshot layer for yrs (Yjs) collaborative state persistence
//!
//! This module provides storage for yrs updates and snapshots, enabling:
//! - Persistence of collaborative document state
//! - Recovery of yrs documents from stored updates
//! - Efficient snapshot compaction
//!
//! ## Architecture
//!
//! Unlike the old CRDT module that tried to implement custom LWW operations,
//! this module simply stores yrs binary updates and snapshots. The actual
//! CRDT logic is handled by the yrs library in grid-server.
//!
//! ```text
//! YrsStore
//!   ├── Store yrs updates (append-only)
//!   ├── Store periodic snapshots (for fast recovery)
//!   └── Compact old updates after snapshot
//! ```

mod store;

pub use store::YrsStore;
