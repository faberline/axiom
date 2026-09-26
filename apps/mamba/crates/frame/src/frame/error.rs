//! Error types for pulsar-frame.

use thiserror::Error;

/// Frame-specific errors.
#[derive(Debug, Error)]
pub enum FrameError {
    /// Column not found in DataFrame.
    #[error("column not found: {0}")]
    ColumnNotFound(String),

    /// Index out of bounds.
    #[error("index out of bounds: {index} (length: {length})")]
    IndexOutOfBounds { index: usize, length: usize },

    /// Label not found in index.
    #[error("label not found in index: {0}")]
    LabelNotFound(String),

    /// Shape mismatch between Series.
    #[error("shape mismatch: expected {expected}, got {actual}")]
    ShapeMismatch { expected: usize, actual: usize },

    /// Type mismatch in operation.
    #[error("type mismatch: {0}")]
    TypeMismatch(String),

    /// Invalid operation.
    #[error("invalid operation: {0}")]
    InvalidOperation(String),

    /// IO error.
    #[error("IO error: {0}")]
    IoError(String),

    /// Array error from array module.
    #[error("array error: {0}")]
    ArrayError(#[from] arraykit::array::ArrayError),
}

/// Result type alias for Frame operations.
pub type Result<T> = std::result::Result<T, FrameError>;
