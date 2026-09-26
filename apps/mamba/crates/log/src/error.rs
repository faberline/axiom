use thiserror::Error;

#[derive(Error, Debug)]
pub enum LogError {
    #[error("Invalid log level: {0}")]
    InvalidLevel(String),

    #[error("Sink error: {0}")]
    SinkError(String),

    #[error("Invalid format string: {0}")]
    InvalidFormat(String),

    #[error("File rotation error: {0}")]
    RotationError(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, LogError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_contract_is_stable() {
        assert_eq!(
            LogError::InvalidLevel("LOUD".to_string()).to_string(),
            "Invalid log level: LOUD"
        );
        assert_eq!(
            LogError::SinkError("closed".to_string()).to_string(),
            "Sink error: closed"
        );
        assert_eq!(
            LogError::InvalidFormat("{bad".to_string()).to_string(),
            "Invalid format string: {bad"
        );
        assert_eq!(
            LogError::RotationError("full".to_string()).to_string(),
            "File rotation error: full"
        );
    }

    #[test]
    fn io_errors_convert_into_log_error() {
        let err: LogError = std::io::Error::other("disk full").into();
        assert_eq!(err.to_string(), "disk full");
    }
}
