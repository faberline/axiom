//! WAL error types

use thiserror::Error;

/// WAL operation result type
pub type Result<T> = std::result::Result<T, WalError>;

/// WAL errors
#[derive(Error, Debug)]
pub enum WalError {
    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Corrupted WAL entry
    #[error("Corrupted WAL entry at position {pos}: {reason}")]
    Corrupted { pos: u64, reason: String },

    /// Checksum mismatch
    #[error("Checksum mismatch at position {pos}: expected {expected:08x}, got {actual:08x}")]
    ChecksumMismatch {
        pos: u64,
        expected: u32,
        actual: u32,
    },

    /// JSON serialization error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Invalid magic number
    #[error("Invalid magic number: expected {expected:?}, got {actual:?}")]
    InvalidMagic { expected: Vec<u8>, actual: Vec<u8> },

    /// Unsupported version
    #[error("Unsupported WAL version: {0}")]
    UnsupportedVersion(u32),

    /// Data directory error
    #[error("Data directory error: {0}")]
    DataDirectory(String),
}
