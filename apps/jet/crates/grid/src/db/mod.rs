//! # jet-grid db module
//!
//! High-performance spreadsheet database layer with Morton encoding and WAL-backed persistence.
//!
//! ## Architecture
//!
//! This module provides:
//! - **Storage Layer**: Efficient cell storage using Morton encoding (Z-order curve)
//! - **Query Layer**: Range queries and spatial queries for spreadsheet operations
//! - **Snapshot Layer**: yrs (Yjs) snapshot storage for collaborative editing
//!
//! ## Components
//!
//! - `storage`: Cell storage engine with WAL support
//! - `query`: Query builders for range and spatial queries
//! - `snapshot`: yrs update/snapshot persistence

pub mod query;
pub mod snapshot;
pub mod storage;

use thiserror::Error;

/// Result type for grid-db operations
pub type Result<T> = std::result::Result<T, SheetDbError>;

/// Error types for grid-db operations
#[derive(Error, Debug)]
pub enum SheetDbError {
    /// Storage layer error
    #[error("Storage error: {0}")]
    Storage(String),

    /// Query execution error
    #[error("Query error: {0}")]
    Query(String),

    /// Snapshot error
    #[error("Snapshot error: {0}")]
    Snapshot(String),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// WAL error
    #[error("WAL error: {0}")]
    Wal(#[from] jet_wal::WalError),

    /// Serialization/deserialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),

    /// JSON serialization error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Invalid input error
    #[error("Invalid input: {0}")]
    InvalidInput(String),
}

// Re-export commonly used types
pub use query::{RangeQuery, SpatialQuery};
pub use snapshot::YrsStore;
pub use storage::{CellStore, MortonKey, StoredCell};
