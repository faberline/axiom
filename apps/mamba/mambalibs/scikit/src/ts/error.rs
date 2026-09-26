//! Time series error types.

use thiserror::Error;

/// Errors that can occur in time series operations.
#[derive(Debug, Error)]
pub enum TsError {
    #[error("insufficient data: need at least {need}, got {got}")]
    InsufficientData { need: usize, got: usize },

    #[error("invalid parameter: {0}")]
    InvalidParameter(String),

    #[error("convergence failed after {0} iterations")]
    ConvergenceFailed(usize),

    #[error("singular matrix encountered")]
    SingularMatrix,

    #[error("frame error: {0}")]
    FrameError(#[from] frame::frame::FrameError),

    #[error("array error: {0}")]
    ArrayError(#[from] arraykit::array::ArrayError),
}

pub type Result<T> = std::result::Result<T, TsError>;
