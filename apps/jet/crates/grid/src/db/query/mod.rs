//! Query layer for spreadsheet operations
//!
//! Provides high-level query builders for common spreadsheet operations:
//! - Range queries (rectangular regions)
//! - Spatial queries (nearest neighbors, clustering)
//! - Filtered queries (by value, formula, etc.)

pub mod range;
pub mod spatial;

pub use range::RangeQuery;
pub use spatial::SpatialQuery;
