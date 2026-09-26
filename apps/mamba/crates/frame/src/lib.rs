//! # frame
//!
//! DataFrame and Series (Pandas-like).

pub mod frame;

// Re-export commonly used types
pub use frame::{DataFrame, Series};
